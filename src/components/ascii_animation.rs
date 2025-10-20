use gloo_timers::callback::Interval;
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

#[function_component(AsciiAnimation)]
pub fn ascii_animation(props: &AsciiAnimationProps) -> Html {
    let frames = use_state(Vec::<String>::new);
    let current_index = use_reducer(|| FrameIndex(0));
    let interval = use_state(|| None::<Interval>);

    // Load frames when component mounts or frame_folder changes
    {
        let frames = frames.clone();
        let current_index = current_index.clone();
        let frame_folder = props.frame_folder.clone();
        
        use_effect_with(frame_folder.clone(), move |_| {
            // Reset index when loading new frames
            current_index.dispatch(AnimationAction::Reset);
            
            web_sys::console::log_1(&"ASCIIAnimation: Loading frames...".into());
            if let Some(frame_contents) = ASCII_PROJECTS.get(frame_folder.as_str()) {
                web_sys::console::log_1(&format!("Found {} frames.", frame_contents.len()).into());
                
                // Convert the static strings to owned Strings
                let owned_frames: Vec<String> = frame_contents.iter().map(|s| s.to_string()).collect();
                frames.set(owned_frames);
            } else {
                web_sys::console::error_1(&format!("No project found for '{}'", frame_folder).into());
            }
            
            || ()
        });
    }

    // Setup animation interval
    {
        let current_index = current_index.clone();
        let interval = interval.clone();
        let frames_len = frames.len();
        let fps = props.fps;
        let loop_anim = props.loop_anim;

        use_effect_with((frames_len, fps, loop_anim), move |(frames_len, fps, loop_anim)| {
            // Clear any existing interval
            interval.set(None);
            
            if *frames_len > 0 {
                let frames_len = *frames_len;
                let fps = *fps;
                let loop_anim = *loop_anim;
                
                let interval_ms = 1000 / fps.max(1);
                
                let new_interval = Interval::new(interval_ms, move || {
                    current_index.dispatch(AnimationAction::NextFrame {
                        total_frames: frames_len,
                        loop_anim,
                    });
                });
                
                interval.set(Some(new_interval));
            }

            move || {
                // Cleanup: drop the interval when effect re-runs or component unmounts
                drop(interval);
            }
        });
    }

    if frames.is_empty() {
        html! { 
            <div class={props.class.clone()}>
                {"Loading ASCII frames..."}
            </div> 
        }
    } else {
        let frame_text = frames.get(current_index.0).cloned().unwrap_or_default();
        
        html! {
            <pre
                class={props.class.clone()}
                style="white-space: pre; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace; line-height: 1; font-size: 10px; width: 100%; max-width: 100%; overflow-x: hidden; overflow-y: hidden;"
            >
                { frame_text }
            </pre>
        }
    }
}
