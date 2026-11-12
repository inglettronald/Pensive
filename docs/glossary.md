# Glossary

Words that mean something specific in Pensive. Where a term also exists in GPUI or in testing generally, the entry says how ours relates.

| Term | Meaning |
|---|---|
| Model | An entity in `model` that holds app state and does I/O through a backend. Not a GPUI type: GPUI's old `Model<T>` is now `Entity<T>`. |
| View | An entity in `ui` that implements `Render`. Reads models and owns its view state. |
| Component | A reusable piece in `components`, usually `RenderOnce`. Takes app data as props and reports changes through callbacks. It may keep temporary UI state such as open or hovered, but never sees models. |
| Element | What views and components render into, such as `div()` and text. GPUI's lowest level. |
| Mock | A view or component rendered in one named state from fixtures. Mock as in mockup, not a test double. |
| Gallery | A component's mock that shows all its states in a labeled grid. |
| Atlas | Every mock of one subject in a numbered grid. `cargo mocks` writes it to `target/mocks/<subject>.png` when it renders all of them. A gallery is one mock. An atlas is built from several. |
| Mock browser | A screen in `cargo demo` that lists every mock by subject, with its thumbnail, and opens any of them in the current window. Ctrl Shift M shows it from anywhere. Its grid for one subject matches that subject's atlas. |
| Notice | What Pensive tells the user about how something went: a success, warning or error, tagged with where it came from (UI, Git, Config or Fixture). It shows on the control it's about, or as a toast and in the badge's list. |
| Fixture | Fixed sample data for mocks, tests and `cargo demo`, in `model` behind a `fixtures` feature. |
| Fake | A test double that stands in for backend logic, such as a `GitBackend` that answers from fixtures. Never called a mock. |
