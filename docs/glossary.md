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
| Fixture | Fixed sample data for mocks and tests, in `model` behind a `fixtures` feature. |
| Fake | A test double that stands in for backend logic, such as a `GitBackend` that answers from fixtures. Never called a mock. |
