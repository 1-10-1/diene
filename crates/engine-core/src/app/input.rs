use std::collections::HashSet;

use winit::{
    event::{ElementState, KeyEvent},
    keyboard::PhysicalKey,
};

#[derive(Debug, Default)]
pub(super) struct InputState {
    held: HashSet<PhysicalKey>,
}

impl InputState {
    pub(super) fn handle_key_event(&mut self, event: &KeyEvent) {
        match event.state {
            ElementState::Pressed => self.held.insert(event.physical_key),
            ElementState::Released => self.held.remove(&event.physical_key),
        };
    }

    pub(super) fn clear_state(&mut self) {
        self.held.clear();
    }

    pub(super) fn _is_held(&self, key: PhysicalKey) -> bool {
        self.held.contains(&key)
    }
}
