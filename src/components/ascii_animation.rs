use yew::prelude::*;
use std::rc::Rc;

include!(concat!(env!("OUT_DIR"), "/ascii_frames.rs"));

#[derive(Properties, PartialEq, Clone)]
pub struct AsciiAnimationProps {
    pub frame_folder: String,
    #[prop_or(24)]
    pub fps: u32,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or(true)]
    pub loop_anim: bool,
}

#[derive(Debug, PartialEq, Clone)]
struct FrameIndex(usize);

enum AnimationAction {
    NextFrame { total_frames: usize, loop_anim: bool },
    Reset,
}

impl Reducible for FrameIndex {
    type Action = AnimationAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            AnimationAction::NextFrame { total_frames, loop_anim } => {
                let next = self.0 + 1;
                let new_index = if next < total_frames {
                    next
                } else if loop_anim {
                    0
                } else {
                    self.0
                };
                Rc::new(FrameIndex(new_index))
            }
            AnimationAction::Reset => Rc::new(FrameIndex(0)),
        }
    }
}

