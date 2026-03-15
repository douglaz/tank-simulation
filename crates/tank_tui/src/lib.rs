use tank_core::{PlayerAction, TankSnapshot};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Overview,
    Chemistry,
    Biology,
    Actions,
    Log,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TuiApp {
    pub active_screen: Screen,
    pub snapshot: TankSnapshot,
    pub pending_actions: Vec<PlayerAction>,
}

impl TuiApp {
    pub fn new(snapshot: TankSnapshot) -> Self {
        Self {
            active_screen: Screen::Overview,
            snapshot,
            pending_actions: Vec::new(),
        }
    }

    pub fn queue_action(&mut self, action: PlayerAction) {
        self.pending_actions.push(action);
    }

    pub fn update_snapshot(&mut self, snapshot: TankSnapshot) {
        self.snapshot = snapshot;
    }
}
