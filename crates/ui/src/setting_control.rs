//! The control that changes a declared setting, built from what it shows now.

use components::{ScrollToChange, SegmentedControl, Select, Stepper, Toggle};
use gpui::{AnyElement, IntoElement};
use settings::{AnySetting, ControlState};

/// The control for `setting`. Using it changes the setting, and so does the wheel once the
/// pointer rests on a control with ordered values.
pub(crate) fn setting_control(
    setting: &'static dyn AnySetting,
    state: &ControlState,
) -> AnyElement {
    let id = setting.key();
    let control = match state {
        ControlState::Toggle(on) => {
            return Toggle::new(id, *on)
                .on_change(move |_, _, cx| setting.step(1, cx))
                .into_any_element();
        }
        ControlState::Segmented { labels, selected } => {
            SegmentedControl::new(id, labels.clone(), *selected)
                .on_select(move |index, _, cx| setting.choose(index, cx))
                .into_any_element()
        }
        ControlState::Select {
            label,
            options,
            selected,
        } => Select::new(id, label.clone())
            .options(options.clone(), *selected)
            .on_choose(move |index, _, cx| setting.choose(index, cx))
            .into_any_element(),
        ControlState::Stepper {
            label,
            can_decrease,
            can_increase,
        } => Stepper::new(id, label.clone())
            .limits(*can_decrease, *can_increase)
            .on_step(move |step, _, cx| setting.step_clamped(step, cx))
            .into_any_element(),
    };
    ScrollToChange::new(id, control)
        .on_step(move |step, _, cx| setting.step_clamped(step, cx))
        .into_any_element()
}
