#[derive(Clone)]
pub struct ControllerState<ST>
where
    ST: StateTypes,
{
    buttons_held: std::collections::HashSet<ST::Button>,
    last_buttons_held: std::collections::HashSet<ST::Button>,
    axes: Vec<i16>,
    last_axes: Vec<i16>,
}

impl<ST> ControllerState<ST>
where
    ST: StateTypes,
{
    pub fn new(num_axes: usize) -> Self {
        Self {
            buttons_held: std::collections::HashSet::new(),
            last_buttons_held: std::collections::HashSet::new(),
            axes: vec![0; num_axes],
            last_axes: vec![0; num_axes],
        }
    }

    pub fn is_button_pressed(&self, button: ST::Button) -> bool {
        !self.last_buttons_held.contains(&button) && self.buttons_held.contains(&button)
    }

    pub fn is_button_released(&self, button: ST::Button) -> bool {
        self.last_buttons_held.contains(&button) && !self.buttons_held.contains(&button)
    }

    pub fn is_button_held(&self, button: ST::Button) -> bool {
        self.buttons_held.contains(&button)
    }

    pub fn axis(&self, axis: usize) -> i16 {
        self.axes[axis]
    }

    pub fn axis_delta(&self, axis: usize) -> i16 {
        self.axes[axis] - self.last_axes[axis]
    }

    pub fn is_axis_leaving_threshold(&self, axis: usize, threshold: i16) -> bool {
        (threshold > 0 && self.axes[axis] > threshold && self.last_axes[axis] <= threshold)
            || (threshold < 0 && self.axes[axis] < threshold && self.last_axes[axis] <= threshold)
    }

    pub fn digest(&mut self) {
        self.last_buttons_held = self.buttons_held.clone();
        self.last_axes = self.axes.clone();
    }
}

pub trait StateTypes
where
    Self::Key: std::hash::Hash + Eq + Copy + Clone,
    Self::Button: std::hash::Hash + Eq + Copy + Clone,
{
    type Key;
    type Button;
}

#[derive(Clone)]
pub struct State<ST>
where
    ST: StateTypes,
{
    keys_held: std::collections::HashSet<ST::Key>,
    last_keys_held: std::collections::HashSet<ST::Key>,
    controllers: std::collections::HashMap<u32, ControllerState<ST>>,
}

impl<ST> State<ST>
where
    ST: StateTypes,
{
    pub fn new() -> Self {
        Self {
            last_keys_held: std::collections::HashSet::new(),
            keys_held: std::collections::HashSet::new(),
            controllers: std::collections::HashMap::new(),
        }
    }

    pub fn handle_key_up(&mut self, key: ST::Key) {
        self.keys_held.remove(&key);
    }

    pub fn handle_key_down(&mut self, key: ST::Key) {
        self.keys_held.insert(key);
    }

    pub fn handle_controller_axis_motion(&mut self, id: u32, axis: usize, value: i16) {
        let controller_state = if let Some(controller_state) = self.controllers.get_mut(&id) {
            controller_state
        } else {
            return;
        };
        controller_state.axes[axis] = value;
    }

    pub fn handle_controller_button_up(&mut self, id: u32, button: ST::Button) {
        let controller_state = if let Some(controller_state) = self.controllers.get_mut(&id) {
            controller_state
        } else {
            return;
        };
        controller_state.buttons_held.remove(&button);
    }

    pub fn handle_controller_button_down(&mut self, id: u32, button: ST::Button) {
        let controller_state = if let Some(controller_state) = self.controllers.get_mut(&id) {
            controller_state
        } else {
            return;
        };
        controller_state.buttons_held.insert(button);
    }

    pub fn handle_controller_connected(&mut self, id: u32, num_axes: usize) {
        self.controllers.insert(id, ControllerState::new(num_axes));
    }

    pub fn handle_controller_disconnected(&mut self, id: u32) {
        self.controllers.remove(&id);
    }

    pub fn clear_keys(&mut self) {
        self.keys_held.clear();
    }

    pub fn is_key_pressed(&self, key: ST::Key) -> bool {
        !self.last_keys_held.contains(&key) && self.keys_held.contains(&key)
    }

    pub fn is_key_released(&self, key: ST::Key) -> bool {
        self.last_keys_held.contains(&key) && !self.keys_held.contains(&key)
    }

    pub fn is_key_held(&self, key: ST::Key) -> bool {
        self.keys_held.contains(&key)
    }

    pub fn iter_controllers(&self) -> impl std::iter::Iterator<Item = (&u32, &ControllerState<ST>)> {
        self.controllers.iter()
    }

    pub fn digest(&mut self) {
        self.last_keys_held = self.keys_held.clone();

        for (_, controller) in self.controllers.iter_mut() {
            controller.digest();
        }
    }
}
