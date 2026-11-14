import hashlib,json,pathlib,subprocess,sys,shutil
SDK=pathlib.Path(sys.argv[1])
ROOT=pathlib.Path(sys.argv[2]);ROOT.mkdir(exist_ok=False)
CLI=pathlib.Path(sys.argv[3]);WORKER=pathlib.Path(sys.argv[4])
media_profile = len(sys.argv) > 5 and sys.argv[5] == '--media-projection'
profile = 'media-projection-v1' if media_profile else 'image-projection-v1'
probe = pathlib.Path(sys.argv[6]).resolve(strict=True) if media_profile else None
video = pathlib.Path(sys.argv[7]).resolve(strict=True) if media_profile else None
def save(path,value):path.write_text(json.dumps(value,indent=2),encoding='utf-8')
package=ROOT/'package';package.mkdir()
shutil.copyfile(SDK/'examples/sync-api/target/wasm32-wasip2/release/bgx_example_sync_api.wasm',package/'adapter.wasm')
manifest=json.loads((SDK/'examples/sync-api/manifest.json').read_text());manifest['requiredFeatures'].append(profile);manifest['artifactSha256']=hashlib.sha256((package/'adapter.wasm').read_bytes()).hexdigest()
save(package/'manifest.json',manifest)
(package/'manifest.sha256').write_text(''.join(hashlib.sha256((package/n).read_bytes()).hexdigest()+' '+n+'\n' for n in ['manifest.json','adapter.wasm']))
server=subprocess.Popen([sys.executable,str(SDK/'fixtures/services/sync_api.py'),'--workspace',str(ROOT/'service')]+(['--video-fixture',str(video)] if media_profile else []),stdout=subprocess.PIPE,stderr=(ROOT/'service.stderr.log').open('w'),text=True)
try:
    ready=json.loads(server.stdout.readline());save(ROOT/'service.ready.json',ready)
    assert ready['event']=='ready',ready
    config={'bindingId': 'projection-binding', 'bindingRevision': 1, 'configurationRef': 'projection-config', 'configuration': {'endpointBaseUrl': 'http://127.0.0.1:1', 'credentialRef': None, 'scenario': 'normal', 'delayMs': 0, 'chunkDelayMs': 0, 'streamFormat': 'sse'}, 'trust': {'publishers': [], 'revokedPlugins': [], 'revokedVersions': [], 'revokedKeys': [], 'denylistedHosts': [], 'isFresh': False, 'supportedFeatures': ['stream-events-v1']}, 'network': {'origins': [], 'allowPrivate': True, 'allowEmbeddedAuthorization': False, 'deniedHosts': [], 'deniedNetworks': []}, 'credentials': [], 'media': [], 'mediaTtlMs': 60000, 'transport': {'handshakeTimeoutMs': 5000, 'writeTimeoutMs': 5000, 'shutdownTimeoutMs': 5000, 'cancelGraceMs': 100}, 'probe': None}
    config['network']['origins']=[ready['origin']];config['configuration']['endpointBaseUrl']=ready['origin'];config['trust']['supportedFeatures'].append(profile)
    if media_profile: config['probe']={'executable':str(probe),'timeoutMs':10000,'frameTimestamps':False}
    save(ROOT/'config.json',config);save(ROOT/'initial-config.json',config);work=ROOT/'workspace'
    def command(args,name,expected=0):
        result=subprocess.run([str(CLI)]+args,capture_output=True,text=True,timeout=120)
        save(ROOT/f'{name}.command.json',{'argv':[str(CLI)]+args,'exitCode':result.returncode})
        (ROOT/f'{name}.stdout.json').write_text(result.stdout);(ROOT/f'{name}.stderr.log').write_text(result.stderr)
        assert result.returncode==expected,result.stdout+result.stderr
        return json.loads(result.stdout)
    def invoke(op,name,payload,create=False,expected=0):
        catalog=json.loads((SDK/'protocol/host-feature-catalog.json').read_text())
        enabled=[f['id'] for f in catalog['features'] if f['requiredImports'] and f['id'] in config['trust']['supportedFeatures']]
        slot='video' if payload.get('capabilityKey')=='video.generate' else 'image'
        request={'protocolVersion':2,'operation':op,'profileId':config['bindingId'],'bindingId':config['bindingId'],'capabilitySlot':slot,'requestId':name,'budgetMs':30000,'negotiatedVersion':2,'enabledFeatures':enabled,'payloadJson':json.dumps(payload)}
        save(ROOT/f'{name}.request.json',request)
        args=['caps' if op=='capabilities' else op,'--sdk',str(SDK),'--worker',str(WORKER),'--package',str(package),'--workspace',str(work),'--config',str(ROOT/'config.json'),'--input',str(ROOT/f'{name}.request.json'),'--output',str(work/'receipts'/f'{name}.json')]
        if create:args.append('--create-workspace')
        envelope=command(args,name,expected)
        return json.loads(envelope['dataJson']) if expected == 0 else envelope
    caps=invoke('capabilities','caps',{},True)
    cases=[('image',p) for p in (['flat','equirectangular180','equirectangular360'] if media_profile else ['flat','equirectangular360'])]
    if media_profile: cases.append(('video','equirectangular360'))
    if '--video-only' in sys.argv[8:]: cases=[case for case in cases if case[0]=='video']
    for kind,projection in cases:
        c=next(c for c in caps['capabilityContracts'] if c['capabilityKey']==kind+'.generate');op=c['operations'][0]
        name=kind+'-'+projection if media_profile else projection
        payload={'schemaVersion':2,'modelId':c['modelIds'][0],'capabilityKey':c['capabilityKey'],'operationId':op['id'],'operationVersion':op['version'],'inputSchemaDigest':op['inputSchema']['digest'],'input':{'prompt':'neutral visual projection fixture','strength':0.5,'outputProjection':projection},'media':[],'uploads':[],'configuration':config['configuration']}
        result=invoke('submit',name,payload)
        assert result['kind']=='completed'
        assert len(result['outputs'])==2
        assert len({o['media']['id'] for o in result['outputs']})==2
        assert any(m['metric'].endswith('/reported-cost') and m['unit'].endswith('/credits') for m in result['usage']['measurements'])
        args=['retained','inspect','--package',str(package),'--workspace',str(work),'--config',str(ROOT/'config.json'),'--request-id',name,'--output',str(work/'receipts'/f'{name}.inspect.json')]
        retained=command(args,name+'.inspect')
        assert retained['checkpointResult']['data']==result['data']
        assert len(retained['media'])==2 and len({m['sha256'] for m in retained['media']})==1
        artifacts=result['data']['mediaArtifacts' if media_profile else 'imageArtifacts']
        assert retained['media'][0]['id']==artifacts[0]['mediaId']
        assert artifacts[0]['projection']==projection
        if media_profile: assert all(row['kind']==kind for row in artifacts)
        selection=ROOT/f'{name}.selection.json';save(selection,[m['id'] for m in retained['media']])
        exported=command(['retained','export','--package',str(package),'--workspace',str(work),'--config',str(ROOT/'config.json'),'--request-id',name,'--output',str(work/'receipts'/f'{name}.export.json'),'--selection',str(selection),'--destination',str(ROOT/f'{name}-export')],name+'.export')
        raw=pathlib.Path(exported['destination'])/exported['media'][0]['relativePath']
        assert hashlib.sha256(raw.read_bytes()).hexdigest()==retained['media'][0]['sha256']
        print(name,'profile negotiation, CLI, checkpoint, retained export PASS',flush=True)
    if media_profile:
        # The fixture video is 2:1. A square declaration and missing measured video
        # dimensions must both fail through the real broker/CLI, not just JSON tests.
        payload['input']['outputProjection']='equirectangular180'
        bad=invoke('submit','video-wrong-aspect',payload,expected=1)
        assert 'INVALID_INPUT' in json.dumps(bad),bad
        payload['input']['outputProjection']='equirectangular360'
        config['probe']=None;save(ROOT/'config.json',config)
        bad=invoke('submit','video-missing-probe',payload,expected=1)
        assert 'INVALID_INPUT' in json.dumps(bad),bad
        config['probe']={'executable':str(probe),'timeoutMs':10000,'frameTimestamps':False};save(ROOT/'config.json',config)
        print('video wrong aspect and absent measured geometry rejected PASS',flush=True)
    save(ROOT/'identities.json',{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [CLI,WORKER,package/'adapter.wasm',package/'manifest.json']})
finally:
    server.terminate();server.wait(timeout=10)
