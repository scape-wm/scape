use crate::{workspace_window::WorkspaceWindow, State};
use smithay::{
    backend::renderer::element::RenderElementStates,
    delegate_presentation,
    desktop::{
        utils::{
            surface_presentation_feedback_flags_from_states, surface_primary_scanout_output,
            OutputPresentationFeedback,
        },
        Space,
    },
    output::Output,
};

delegate_presentation!(State);

// TODO: find a new home for this code
#[cfg_attr(feature = "profiling", profiling::function)]
pub fn take_presentation_feedback(
    output: &Output,
    space: &Space<WorkspaceWindow>,
    render_element_states: &RenderElementStates,
) -> OutputPresentationFeedback {
    let mut output_presentation_feedback = OutputPresentationFeedback::new(output);

    for window in space.elements() {
        match window {
            WorkspaceWindow::ApplicationWindow(w)
                if space.outputs_for_element(window).contains(output) =>
            {
                w.take_presentation_feedback(
                    &mut output_presentation_feedback,
                    surface_primary_scanout_output,
                    |surface, _| {
                        surface_presentation_feedback_flags_from_states(
                            surface,
                            render_element_states,
                        )
                    },
                );
            }
            _ => {}
        }
    }

    let map = smithay::desktop::layer_map_for_output(output);
    for layer_surface in map.layers() {
        layer_surface.take_presentation_feedback(
            &mut output_presentation_feedback,
            surface_primary_scanout_output,
            |surface, _| {
                surface_presentation_feedback_flags_from_states(surface, render_element_states)
            },
        );
    }

    output_presentation_feedback
}
