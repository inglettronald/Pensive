//! The control that changes a declared setting, built from what it shows now.

use components::{SegmentedControl, Select, Stepper, Toggle};
use gpui::{AnyElement, IntoElement};
use settings::{AnySetting, ControlState};

/// A toggle, segmented control, select or stepper for `setting`. Using it changes the setting.
pub(crate) fn setting_control(
    setting: &'static dyn AnySetting,
    state: &ControlState,
) -> AnyElement {
    let id = setting.key();
    match state {
        ControlState::Toggle(on) => Toggle::new(id, *on)
            .on_change(move |_, _, cx| setting.step(1, cx))
            .into_any_element(),
        ControlState::Segmented { labels, selected } => {
            SegmentedControl::new(id, labels.clone(), *selected)
                .on_select(move |index, _, cx| setting.choose(index, cx))
                .into_any_element()
        }
        ControlState::Select { label } => Select::new(id, label.clone())
            .on_click(move |_, cx| setting.step(1, cx))
            .into_any_element(),
        ControlState::Stepper {
            label,
            can_decrease,
            can_increase,
        } => Stepper::new(id, label.clone())
            .limits(*can_decrease, *can_increase)
            .on_step(move |step, _, cx| setting.step(step, cx))
            .into_any_element(),
    }
}
