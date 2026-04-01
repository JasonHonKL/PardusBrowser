

#[derive(Debug, Clone)]
pub struct TimerEntry {
    pub id: u32,
    pub callback_str: Option<String>,
    pub delay_ms: u64,
    pub is_interval: bool,
    pub is_fired: bool,
}

#[derive(Debug)]
pub struct TimerQueue {
    timers: Vec<TimerEntry>,
    next_id: u32,
    max_ticks: u32,
    tick_count: u32,
}

impl TimerQueue {
    pub fn new() -> Self {
        Self {
            timers: Vec::new(),
            next_id: 1,
            max_ticks: 1000,
            tick_count: 0,
        }
    }

    pub fn set_timeout(&mut self, callback_str: Option<String>, delay_ms: u64) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.timers.push(TimerEntry {
            id,
            callback_str,
            delay_ms,
            is_interval: false,
            is_fired: false,
        });
        id
    }

    pub fn set_interval(&mut self, callback_str: Option<String>, delay_ms: u64) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.timers.push(TimerEntry {
            id,
            callback_str,
            delay_ms,
            is_interval: true,
            is_fired: false,
        });
        id
    }

    pub fn clear_timer(&mut self, id: u32) {
        if let Some(pos) = self.timers.iter().position(|t| t.id == id) {
            self.timers.remove(pos);
        }
    }

    pub fn tick_count(&self) -> u32 {
        self.tick_count
    }

    pub fn is_at_limit(&self) -> bool {
        self.tick_count >= self.max_ticks
    }

    pub fn get_expired_timer_callbacks_js(&self) -> String {
        let mut js = String::new();
        for timer in &self.timers {
            if timer.is_fired {
                continue;
            }
            if timer.delay_ms == 0 {
                if let Some(cb) = &timer.callback_str {
                    js.push_str(&format!(
                        "try {{ (function() {{ {} }})(); }} catch(e) {{ }}\n",
                        cb
                    ));
                }
            }
        }
        js
    }

    pub fn mark_delay_zero_fired(&mut self) {
        for timer in &mut self.timers {
            if timer.delay_ms == 0 && !timer.is_interval && !timer.is_fired {
                timer.is_fired = true;
                self.tick_count += 1;
            }
        }
    }
}
