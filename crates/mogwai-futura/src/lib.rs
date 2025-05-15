//!
//!
//! Model, view, logic.
//!
//! ## Model
//! Model is some data that can set _every_ slot in the view.
//! The type of the model cannot change from platform to platform.
//!
//! ## View Interface
//! An interface for interacting with the view.
//! The type of the interface cannot change from platform to platform.
//!
//! ## Logic
//! The logic is the computation that takes changes from the view through the interface,
//! updates the model and applies changes back through the interface.
//!
//! ## View
//! The view itself is responsible for rendering.
//! The type of the view changes depending on the platform.
//!

pub mod tuple;

mod button_clicks {
    use std::pin::Pin;

    pub enum ButtonClickEvent {
        Clicked,
    }

    pub struct ButtonClicks {
        pub clicks: u32,
    }

    pub trait ButtonClicksInterface {
        fn set_clicks(&self, clicks: u32);
        fn get_next_event(&self) -> Pin<Box<dyn Future<Output = ButtonClickEvent>>>;
    }

    impl ButtonClicks {
        pub async fn run(&mut self, interface: impl ButtonClicksInterface) {
            match interface.get_next_event().await {
                ButtonClickEvent::Clicked => {
                    self.clicks += 1;
                    interface.set_clicks(self.clicks);
                }
            }
        }
    }

    // TODO: web-sys impl of ButtonClicksInterface
    // TODO: ratatui impl of ButtonClicksInterface
}
