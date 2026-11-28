# Design

Pensive is where you read changes and decide whether they're right, often changes an agent made. The app should feel calm while you do that: dark, still, and out of the way until you reach for it.

This page describes that feel, and every new design gets checked against it. What a screen does lives in its code and mocks. Exact colors, fonts and sizes live in the `theme` crate, and `cargo mocks theme` renders them. To draft a design, run the `design` skill.

## The code comes first

When the code and the interface compete for attention, the code wins. Syntax colors stay readable under diff marks. The Commit button is outlined instead of filled, so it doesn't outshine the changes it commits.

## Color carries meaning

The base is neutral gray, and Pensive is dark by default. Pensive Light keeps the grays neutral for a bright room, and Pensive Paper turns them sepia, with brown ink, so no surface is white. Color shows up only when it tells you something. Green, red and amber report changes and how things went. One blue marks interface signals. If taking a color away loses no information, take it away.

## Quiet until something matters

An idle window shows no progress bar and no notice badge. Errors can interrupt you, and so can long work finishing. Warnings wait in the badge, so a noisy source can't take over the screen.

## Close to the work

Actions and feedback sit next to what they're about. Git actions live in the Changes panel, beside the changes. A problem with a control shows on that control, and a toast is for news with no control to point at.

## Room to read

Navigation keeps to the top and left edges, so content is never boxed in from both sides. There is no status bar. A long path or commit title fades at the edge, so it never ends in a row of dots.

## Nothing changes by accident

Scrolling past a setting leaves it alone, because a control only takes the wheel after you rest on it. Saving stops if the file changed on disk, so Pensive never overwrites work it hasn't seen.

## Learn it once

The same thing works the same way everywhere. Ctrl K opens the palette from any screen, even before a project is open. The palette's tabs stay fixed, and plugins add keywords instead of more tabs. An icon switcher in a header is the segmented control from Settings at a smaller size.

## Motion shows where things went

Motion explains a change and then stops. A segmented control's selection stretches to its new segment. When the rail narrows, its names slide out under its edge.

## Taste is a setting

When a visual choice comes down to taste, it becomes a setting with an opinionated default. The defaults should look finished if nobody touches them. Calm is the default density, and Dense is one setting away. Pensive Light and Pensive Paper ship beside Pensive Dark, and anything else can be a user theme.
