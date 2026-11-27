//! Interpret an installed transcription adapter's declared execution policy.
use serde_json::Value;
use std::{collections::VecDeque, ops::Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    ChatMessage,
    ContinuousDictation,
}

impl Usage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChatMessage => "chatMessage",
            Self::ContinuousDictation => "continuousDictation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    max_samples: u64,
    progressive: Option<(u64, u64)>,
}

impl Policy {
    pub fn parse(usage: Usage, value: &Value) -> Result<Self, &'static str> {
        let invalid = "invalid transcription policy";
        let integer = |name| value[name].as_u64()
            .filter(|&n| (1..=9_007_199_254_740_991).contains(&n)).ok_or(invalid);
        if value["schemaVersion"] != 1 || value["usage"] != usage.as_str() {
            return Err(invalid);
        }
        let max_samples = integer("maxBatchSamples")?;
        let size = value.as_object().map(|object| object.len());
        let progressive = match (usage, value["strategy"].as_str(), size) {
            (Usage::ChatMessage, Some("wholeFile"), Some(4)) => None,
            (Usage::ContinuousDictation, Some("progressiveFile"), Some(6)) => {
                let min_samples = integer("minPauseFlushSamples")?;
                if min_samples > max_samples {
                    return Err(invalid);
                }
                Some((min_samples, integer("maxBufferedMs")?))
            }
            _ => return Err(invalid),
        };
        Ok(Self { max_samples, progressive })
    }

    pub fn max_samples(self) -> u64 {
        self.max_samples
    }

    pub fn planner(self) -> Planner {
        Planner { policy: self, start: 0, end: 0, pauses: VecDeque::new(),
            deadline: None, finished: false }
    }
}

pub struct Planner {
    policy: Policy,
    start: u64,
    end: u64,
    pauses: VecDeque<u64>,
    deadline: Option<u64>,
    finished: bool,
}

impl Planner {
    pub fn deadline_ms(&self) -> Option<u64> {
        self.deadline
    }

    /// Pause offsets may include the preceding block's end when VAD confirms a pause.
    pub fn push(&mut self, end: u64, pauses: &[u64], now_ms: u64)
        -> Result<Vec<Range<u64>>, &'static str>
    {
        if self.finished || end < self.end
            || pauses.iter().any(|&p| p == 0 || p < self.end || p > end)
            || pauses.windows(2).any(|p| p[0] >= p[1])
            || self.pauses.back().zip(pauses.first()).is_some_and(|(a, b)| a >= b)
        {
            return Err("invalid transcription sample offsets");
        }
        if self.start == self.end && self.end < end {
            self.arm(now_ms);
        }
        self.end = end;
        self.pauses.extend(pauses.iter().copied().filter(|&p| p > self.start));
        Ok(self.poll(now_ms))
    }

    pub fn poll(&mut self, now_ms: u64) -> Vec<Range<u64>> {
        if self.policy.progressive.is_none() || self.finished {
            return Vec::new();
        }
        self.take_ranges(now_ms, false)
    }

    pub fn finish(&mut self) -> Vec<Range<u64>> {
        self.finished = true;
        let result = self.take_ranges(0, true);
        self.deadline = None;
        result
    }

    fn arm(&mut self, now_ms: u64) {
        self.deadline = self.policy.progressive.map(|(_, ms)| now_ms.saturating_add(ms));
    }

    fn take_ranges(&mut self, now_ms: u64, finishing: bool) -> Vec<Range<u64>> {
        let mut ranges = Vec::new();
        while self.start < self.end {
            let limit = self.start.saturating_add(self.policy.max_samples);
            let pause = self.policy.progressive.and_then(|(min, _)| {
                self.pauses.iter().copied()
                    .find(|&p| p <= limit && p - self.start >= min)
            });
            let cut = if let Some(pause) = pause {
                pause
            } else if self.end > limit || (self.end == limit && self.policy.progressive.is_some()) {
                self.pauses.iter().copied().take_while(|&p| p <= limit).last().unwrap_or(limit)
            } else if finishing || self.deadline.is_some_and(|deadline| now_ms >= deadline) {
                self.end
            } else {
                break;
            };
            ranges.push(self.start..cut);
            self.start = cut;
            while self.pauses.front().is_some_and(|&p| p <= cut) {
                self.pauses.pop_front();
            }
            if !finishing && self.start < self.end {
                self.arm(now_ms);
            } else {
                self.deadline = None;
            }
        }
        ranges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn transcription_policy_chat_waits_for_finish_and_covers_input() {
        let policy = Policy::parse(Usage::ChatMessage, &json!({"schemaVersion":1,"usage":"chatMessage",
            "strategy":"wholeFile","maxBatchSamples":10})).unwrap();
        for (samples, cuts) in [(0, vec![]), (1, vec![0..1]), (9, vec![0..9]), (10, vec![0..10]),
            (11, vec![0..10, 10..11]), (22, vec![0..10, 10..20, 20..22])] {
            let mut planner = policy.planner();
            assert!(planner.push(samples, &[], 0).unwrap().is_empty());
            assert_eq!(planner.finish(), cuts);
            assert!(planner.finish().is_empty());
        }
        let mut planner = policy.planner();
        planner.push(22, &[8, 16], 0).unwrap();
        assert_eq!(planner.finish(), vec![0..8, 8..16, 16..22]);
    }

    #[test]
    fn transcription_policy_progressive_flushes_without_new_audio() {
        let policy = Policy::parse(Usage::ContinuousDictation, &json!({"schemaVersion":1,
            "usage":"continuousDictation","strategy":"progressiveFile","maxBatchSamples":10,
            "minPauseFlushSamples":4,"maxBufferedMs":8})).unwrap();
        let mut planner = policy.planner();
        assert!(planner.push(3, &[3], 100).unwrap().is_empty());
        assert_eq!(planner.deadline_ms(), Some(108));
        assert!(planner.poll(107).is_empty());
        assert_eq!(planner.poll(108), vec![0..3]);
        assert_eq!(planner.push(8, &[8], 110).unwrap(), vec![3..8]);
        assert_eq!(planner.push(28, &[], 111).unwrap(), vec![8..18, 18..28]);
        assert!(planner.push(29, &[27], 112).is_err());
        assert!(planner.push(29, &[], 113).unwrap().is_empty());
        assert_eq!(planner.finish(), vec![28..29]);
    }
}

