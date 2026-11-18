//! Finds layout problems in what a frame painted: content cut off or overlapping, a centered group
//! off center, a main area or text squeezed too narrow, and text fading where it has no need to.

use image::{Rgba, RgbaImage};

use crate::{
    headless::SCALE,
    painted::{Fill, Ink, Painted, PaintedBox, Rect},
};

/// How far past an edge something must reach to count, in window pixels. Glyph bounds carry a
/// little padding, and rounded layout lands within a pixel.
const TOLERANCE: f32 = 1.;
/// Flagged rectangles this close together are reported as one.
const MERGE_GAP: f32 = 8.;
/// How much of the smaller glyph two glyphs must share to count as overlapping. Neighbors in a
/// word touch, but never by this much.
const MIN_OVERLAP: f32 = 0.25;
/// Two glyphs must also share at least this much each way, in window pixels, since a small mark
/// like a period can sit almost entirely inside its neighbor's box.
const MIN_OVERLAP_SIDE: f32 = 3.;
/// How far a centered group may drift, in window pixels.
const CENTER_TOLERANCE: f32 = 2.;
/// The narrowest main area that's still usable, at 100% zoom: about 38 columns of Calm's code.
const MIN_MAIN_WIDTH: f32 = 320.;
/// A text fade is at most this wide at 100% zoom, and at least this wide in window pixels, which
/// leaves out thin lines that fade along their length.
const MAX_FADE_WIDTH: f32 = 40.;
const MIN_FADE_WIDTH: f32 = 8.;
/// A fade over more than this share of its text's box leaves too little to read.
const MAX_FADE_SHARE: f32 = 0.5;
/// Two boxes this close in width are taken for the same box at another window size.
const SAME_WIDTH: f32 = 2.;
/// Text wrapping after every letter shows as at least this many letters in a column.
const MIN_LETTER_STACK: usize = 5;
/// A letter is at most this big at 100% zoom, which leaves out the rail's icons.
const MAX_LETTER_WIDTH: f32 = 14.;
const MAX_LETTER_HEIGHT: f32 = 16.;
/// The lines of wrapped text are at most this far apart at 100% zoom.
const MAX_LINE_PITCH: f32 = 32.;
/// Letters of one column start within this much of each other at 100% zoom.
const MAX_LETTER_DRIFT: f32 = 4.;
/// A letter with another this many of its widths away is part of a word.
const WORD_GAP: f32 = 3.;
/// Columns docked at the window's left edge are at least this share of its height.
const MIN_COLUMN_HEIGHT: f32 = 0.5;
/// A title bar is at most this share of the window's height.
const MAX_TITLE_BAR_HEIGHT: f32 = 0.25;

const MARK: Rgba<u8> = Rgba([255, 0, 255, 255]);
/// The width of the outline around a flag, in image pixels.
const MARK_WIDTH: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Clipped,
    Overlap,
    OffCenter,
    Squeezed,
    Fading,
}

impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Clipped,
        Self::Overlap,
        Self::OffCenter,
        Self::Squeezed,
        Self::Fading,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Clipped => "clipped",
            Self::Overlap => "overlap",
            Self::OffCenter => "off center",
            Self::Squeezed => "squeezed",
            Self::Fading => "fading",
        }
    }
}

/// What a flag found, worst first: from text nobody can read to text that fades needlessly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Problem {
    LettersStacked,
    MainSqueezed,
    PastWindow,
    Overlap,
    OffCenter,
    CutOff,
    MostlyFade,
    NeedlessFade,
}

impl Problem {
    pub const ALL: [Self; 8] = [
        Self::LettersStacked,
        Self::MainSqueezed,
        Self::PastWindow,
        Self::Overlap,
        Self::OffCenter,
        Self::CutOff,
        Self::MostlyFade,
        Self::NeedlessFade,
    ];

    pub fn kind(self) -> Kind {
        match self {
            Self::LettersStacked | Self::MainSqueezed => Kind::Squeezed,
            Self::PastWindow | Self::CutOff => Kind::Clipped,
            Self::Overlap => Kind::Overlap,
            Self::OffCenter => Kind::OffCenter,
            Self::MostlyFade | Self::NeedlessFade => Kind::Fading,
        }
    }

    /// Its name in the list of accepted flags.
    pub fn id(self) -> &'static str {
        match self {
            Self::LettersStacked => "letters-stacked",
            Self::MainSqueezed => "main-squeezed",
            Self::PastWindow => "past-window",
            Self::Overlap => "overlap",
            Self::OffCenter => "off-center",
            Self::CutOff => "cut-off",
            Self::MostlyFade => "mostly-fade",
            Self::NeedlessFade => "needless-fade",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|problem| problem.id() == id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flag {
    pub problem: Problem,
    /// Where, in window pixels.
    pub at: Rect,
    pub detail: String,
}

impl Flag {
    pub fn kind(&self) -> Kind {
        self.problem.kind()
    }
}

/// What a check needs to know beyond the frame itself.
#[derive(Clone, Copy, Debug)]
pub struct Expected<'a> {
    /// In percent.
    pub zoom: u32,
    /// Whether the mock shows a group that belongs in the middle of the window.
    pub centered: bool,
    /// The widths of the boxes whose text fades in the same mock at the reference size.
    pub reference_fades: &'a [f32],
}

/// Everything wrong with `painted`.
pub fn check(painted: &Painted, expected: Expected<'_>) -> Vec<Flag> {
    let zoom = expected.zoom as f32 / 100.;
    let fades = fades(painted, expected.zoom);
    let mut flags = clipped(painted, &fades);
    flags.extend(overlapping(painted));
    if expected.centered {
        flags.extend(off_center(painted));
    }
    flags.extend(squeezed(painted, zoom));
    flags.extend(letter_stacks(painted, zoom));
    flags.extend(fading(&fades, expected.reference_fades));
    flags
}

/// The widths of the boxes whose text fades, to compare another size with.
pub fn fade_widths(painted: &Painted, zoom: u32) -> Vec<f32> {
    fades(painted, zoom)
        .iter()
        .map(|fade| fade.clip.width())
        .collect()
}

/// The boxes painted over the end of text that doesn't fit.
fn fades(painted: &Painted, zoom: u32) -> Vec<PaintedBox> {
    let widest = MAX_FADE_WIDTH * zoom as f32 / 100.;
    painted
        .boxes
        .iter()
        .filter(|fade| {
            let width = fade.bounds.width();
            fade.fill == Fill::Gradient && (MIN_FADE_WIDTH..=widest).contains(&width)
        })
        .copied()
        .collect()
}

/// Draws an outline around each of `flags` on a copy of the mock's image.
pub fn marked(image: &RgbaImage, flags: &[Flag]) -> RgbaImage {
    let mut marked = image.clone();
    let (width, height) = marked.dimensions();
    for flag in flags {
        let pixel = |value: f32, most: u32| ((value * SCALE).max(0.) as u32).min(most);
        let left = pixel(flag.at.left, width);
        let right = pixel(flag.at.right, width);
        let top = pixel(flag.at.top, height);
        let bottom = pixel(flag.at.bottom, height);
        for y in top..bottom {
            for x in left..right {
                let inside = x >= left + MARK_WIDTH
                    && x + MARK_WIDTH < right
                    && y >= top + MARK_WIDTH
                    && y + MARK_WIDTH < bottom;
                if !inside {
                    marked.put_pixel(x, y, MARK);
                }
            }
        }
    }
    marked
}

fn clipped(painted: &Painted, fades: &[PaintedBox]) -> Vec<Flag> {
    let window = painted.window();
    let rows = row_clips(painted);
    let mut past_window = Vec::new();
    let mut cut_off = Vec::new();
    for ink in &painted.ink {
        let shown = ink.bounds.intersect(&ink.clip);
        if ink.clip.contains(&window) {
            if !within(&window, &ink.bounds) {
                past_window.push(shown);
            }
        } else if crosses_side(ink) && !rows.contains(&ink.clip) && !under_fade(ink, fades) {
            cut_off.push(shown);
        }
    }
    let boxes = painted
        .boxes
        .iter()
        .filter(|item| item.clip.contains(&window) && !is_invisible(item))
        .filter(|item| !within(&window, &item.bounds))
        .map(|item| item.bounds.intersect(&window));
    past_window.extend(outermost(boxes.collect()));
    let past_window = merged(past_window);
    cut_off.retain(|cut: &Rect| !past_window.iter().any(|rect| rect.contains(cut)));

    let flags = |rects, problem, detail: &str| -> Vec<Flag> {
        merged(rects)
            .into_iter()
            .map(|at| Flag {
                problem,
                at,
                detail: detail.to_string(),
            })
            .collect()
    };
    let mut found = flags(
        past_window,
        Problem::PastWindow,
        "runs past the window's edge",
    );
    found.extend(flags(cut_off, Problem::CutOff, "cut off with no fade"));
    found
}

/// Whether `inner` stays inside `outer`, give or take the tolerance.
fn within(outer: &Rect, inner: &Rect) -> bool {
    inner.left >= outer.left - TOLERANCE
        && inner.top >= outer.top - TOLERANCE
        && inner.right <= outer.right + TOLERANCE
        && inner.bottom <= outer.bottom + TOLERANCE
}

/// Whether `ink` runs past the left or right of what clips it. Running past the top or bottom is
/// how a scrolled list looks, so it says nothing.
fn crosses_side(ink: &Ink) -> bool {
    ink.bounds.left < ink.clip.left - TOLERANCE || ink.bounds.right > ink.clip.right + TOLERANCE
}

fn under_fade(ink: &Ink, fades: &[PaintedBox]) -> bool {
    let edge = match ink.bounds.right > ink.clip.right {
        true => ink.clip.right,
        false => ink.clip.left,
    };
    fades.iter().any(|fade| {
        fade.order > ink.order
            && edge >= fade.bounds.left - TOLERANCE
            && edge <= fade.bounds.right + TOLERANCE
            && !fade.bounds.intersect(&ink.bounds).is_empty()
    })
}

/// The clips of lines stacked directly on one another, as an editor's are. Their lines are as long
/// as they are and scroll sideways, so running past the edge is normal.
fn row_clips(painted: &Painted) -> Vec<Rect> {
    let mut clips: Vec<Rect> = painted.ink.iter().map(|ink| ink.clip).collect();
    clips.sort_by(|a, b| a.top.total_cmp(&b.top));
    clips.dedup();
    let touching = |a: &Rect, b: &Rect| {
        (a.left - b.left).abs() <= TOLERANCE
            && (a.right - b.right).abs() <= TOLERANCE
            && ((a.bottom - b.top).abs() <= TOLERANCE || (b.bottom - a.top).abs() <= TOLERANCE)
    };
    clips
        .iter()
        .filter(|clip| clips.iter().any(|other| touching(clip, other)))
        .copied()
        .collect()
}

fn is_invisible(item: &PaintedBox) -> bool {
    matches!(item.fill, Fill::Solid(alpha) if alpha <= 0.)
}

/// Drops each rectangle inside another one.
fn outermost(rects: Vec<Rect>) -> Vec<Rect> {
    let mut kept: Vec<Rect> = Vec::new();
    for rect in rects {
        if kept.iter().any(|other| other.contains(&rect)) {
            continue;
        }
        kept.retain(|other| !rect.contains(other));
        kept.push(rect);
    }
    kept
}

/// Joins rectangles that touch or nearly do.
fn merged(mut rects: Vec<Rect>) -> Vec<Rect> {
    let near = |a: &Rect, b: &Rect| {
        a.left <= b.right + MERGE_GAP
            && b.left <= a.right + MERGE_GAP
            && a.top <= b.bottom + MERGE_GAP
            && b.top <= a.bottom + MERGE_GAP
    };
    let mut joined: Vec<Rect> = Vec::new();
    while let Some(mut rect) = rects.pop() {
        while let Some(index) = joined.iter().position(|other| near(&rect, other)) {
            rect = rect.union(&joined.swap_remove(index));
        }
        joined.push(rect);
    }
    joined
}

fn overlapping(painted: &Painted) -> Vec<Flag> {
    let visible: Vec<(Rect, u32)> = painted
        .ink
        .iter()
        .map(|ink| (ink.bounds.intersect(&ink.clip), ink.order))
        .filter(|(shown, _)| !shown.is_empty())
        .collect();
    let mut order: Vec<usize> = (0..visible.len()).collect();
    order.sort_by(|&a, &b| visible[a].0.left.total_cmp(&visible[b].0.left));

    let mut overlaps = Vec::new();
    for (position, &index) in order.iter().enumerate() {
        let (a, a_order) = visible[index];
        for &other in &order[position + 1..] {
            let (b, b_order) = visible[other];
            if b.left >= a.right {
                break;
            }
            let shared = a.intersect(&b);
            let small = shared.width() < MIN_OVERLAP_SIDE || shared.height() < MIN_OVERLAP_SIDE;
            if small || area(&shared) < MIN_OVERLAP * area(&a).min(area(&b)) {
                continue;
            }
            if !is_hidden(painted, &shared, a_order.min(b_order)) {
                overlaps.push(shared);
            }
        }
    }
    merged(overlaps)
        .into_iter()
        .map(|at| Flag {
            problem: Problem::Overlap,
            at,
            detail: "content drawn over other content".to_string(),
        })
        .collect()
}

fn area(rect: &Rect) -> f32 {
    rect.width() * rect.height()
}

/// Whether a box painted after `order` covers `rect`, so what `order` drew there can't be seen.
fn is_hidden(painted: &Painted, rect: &Rect, order: u32) -> bool {
    painted.boxes.iter().any(|item| {
        item.order > order && item.is_opaque() && item.bounds.intersect(&item.clip).contains(rect)
    })
}

/// The group should sit in the middle of the window when it floats over a scrim, or of the space
/// under the title bar when it's the whole screen.
fn off_center(painted: &Painted) -> Option<Flag> {
    let window = painted.window();
    let scrim = painted
        .boxes
        .iter()
        .find(|item| item.bounds.contains(&window) && !item.is_opaque() && !is_invisible(item));
    let (area, group) = match scrim {
        Some(scrim) => (window, group_after(painted, scrim.order)?),
        None => {
            let below = Rect {
                top: title_bar_bottom(painted),
                ..window
            };
            (below, group_within(painted, &below)?)
        }
    };
    let across = (group.left + group.right - area.left - area.right) / 2.;
    let down = (group.top + group.bottom - area.top - area.bottom) / 2.;
    (across.abs() > CENTER_TOLERANCE || down.abs() > CENTER_TOLERANCE).then(|| Flag {
        problem: Problem::OffCenter,
        at: group.intersect(&window),
        detail: format!("{across:+.0} px across and {down:+.0} px down from the center"),
    })
}

fn group_after(painted: &Painted, order: u32) -> Option<Rect> {
    let boxes = painted
        .boxes
        .iter()
        .filter(|item| item.order > order && !is_invisible(item))
        .map(|item| item.bounds.intersect(&item.clip));
    let ink = painted
        .ink
        .iter()
        .filter(|ink| ink.order > order)
        .map(|ink| ink.bounds.intersect(&ink.clip));
    boxes.chain(ink).reduce(|a, b| a.union(&b))
}

fn group_within(painted: &Painted, area: &Rect) -> Option<Rect> {
    let boxes = painted
        .boxes
        .iter()
        .filter(|item| !is_invisible(item) && !item.bounds.contains(area))
        .map(|item| item.bounds.intersect(&item.clip));
    let ink = painted
        .ink
        .iter()
        .map(|ink| ink.bounds.intersect(&ink.clip));
    boxes
        .chain(ink)
        .filter(|rect| rect.top >= area.top)
        .reduce(|a, b| a.union(&b))
}

/// The bottom of the shortest box across the top of the window.
fn title_bar_bottom(painted: &Painted) -> f32 {
    let tallest = painted.height * MAX_TITLE_BAR_HEIGHT;
    painted
        .boxes
        .iter()
        .filter(|item| !is_invisible(item))
        .filter(|item| item.bounds.left <= TOLERANCE && item.bounds.top <= TOLERANCE)
        .filter(|item| item.bounds.right >= painted.width - TOLERANCE)
        .map(|item| item.bounds.bottom)
        .filter(|&bottom| bottom <= tallest)
        .fold(0., f32::max)
}

/// The main area is what the columns docked at the left leave of the window. A box ending right
/// at the window's edge is the main area itself, while a column can run past that edge.
fn squeezed(painted: &Painted, zoom: f32) -> Option<Flag> {
    let tall = painted.height * MIN_COLUMN_HEIGHT;
    let mut columns: Vec<Rect> = painted
        .boxes
        .iter()
        .filter(|item| item.is_opaque() && item.bounds.height() >= tall)
        .filter(|item| (item.bounds.bottom - painted.height).abs() <= TOLERANCE)
        .filter(|item| (item.bounds.right - painted.width).abs() > TOLERANCE)
        .map(|item| item.bounds)
        .collect();
    columns.sort_by(|a, b| a.left.total_cmp(&b.left));
    let mut edge = 0_f32;
    for column in columns {
        if column.left <= edge + TOLERANCE {
            edge = edge.max(column.right);
        }
    }
    if edge <= 0. {
        return None;
    }
    let width = ((painted.width - edge) / zoom).max(0.);
    (width < MIN_MAIN_WIDTH).then(|| Flag {
        problem: Problem::MainSqueezed,
        at: Rect::new(edge.min(painted.width), 0., painted.width, painted.height),
        detail: format!("main area is {width:.0} px wide at 100% zoom, under {MIN_MAIN_WIDTH:.0}"),
    })
}

/// Text so short of room that it wraps after every letter. Each letter has nothing after it on
/// its line, unlike a line number, which has its code. An editor's lines don't wrap, so they're
/// left out, as what's left of them at a narrow edge can look the same.
fn letter_stacks(painted: &Painted, zoom: f32) -> Vec<Flag> {
    let rows = row_clips(painted);
    let mut shown: Vec<(Rect, bool)> = painted
        .ink
        .iter()
        .map(|ink| {
            let whole = ink.clip.contains(&ink.bounds) && !rows.contains(&ink.clip);
            (ink.bounds.intersect(&ink.clip), whole)
        })
        .filter(|(rect, _)| !rect.is_empty())
        .collect();
    shown.sort_by(|a, b| a.0.top.total_cmp(&b.0.top));
    let (shown, whole): (Vec<Rect>, Vec<bool>) = shown.into_iter().unzip();
    let is_letter = |rect: &Rect| {
        rect.width() <= MAX_LETTER_WIDTH * zoom && rect.height() <= MAX_LETTER_HEIGHT * zoom
    };
    let mut stacks: Vec<(Rect, Rect, usize)> = Vec::new();
    for (index, letter) in shown.iter().enumerate() {
        if !whole[index] || !is_letter(letter) || !alone_on_line(&shown, index) {
            continue;
        }
        let continued = stacks.iter_mut().find(|(_, last, _)| {
            (letter.left - last.left).abs() <= MAX_LETTER_DRIFT * zoom
                && letter.top - last.top <= MAX_LINE_PITCH * zoom
        });
        match continued {
            Some((whole, last, count)) => {
                *whole = whole.union(letter);
                *last = *letter;
                *count += 1;
            }
            None => stacks.push((*letter, *letter, 1)),
        }
    }
    stacks
        .into_iter()
        .filter(|(_, _, count)| *count >= MIN_LETTER_STACK)
        .map(|(at, _, _)| Flag {
            problem: Problem::LettersStacked,
            at,
            detail: "text wraps after every letter".to_string(),
        })
        .collect()
}

/// Whether the glyph at `index` of `shown`, which is sorted by top, has nothing after it on its
/// line and no neighbor close before it.
fn alone_on_line(shown: &[Rect], index: usize) -> bool {
    let letter = &shown[index];
    let reach = letter.height() * 2.;
    let later = shown[index + 1..]
        .iter()
        .take_while(|other| other.top < letter.bottom);
    let earlier = shown[..index]
        .iter()
        .rev()
        .take_while(|other| other.top > letter.top - reach);
    !later.chain(earlier).any(|other| {
        let shared = other.bottom.min(letter.bottom) - other.top.max(letter.top);
        shared > other.height().min(letter.height()) / 2.
            && other.right > letter.left - letter.width() * WORD_GAP
    })
}

fn fading(fades: &[PaintedBox], reference: &[f32]) -> Vec<Flag> {
    let has_room_elsewhere = |width: f32| {
        !reference
            .iter()
            .any(|faded| (faded - width).abs() <= SAME_WIDTH)
    };
    fades
        .iter()
        .filter_map(|fade| {
            let width = fade.clip.width();
            let (problem, detail) = if fade.bounds.width() > width * MAX_FADE_SHARE {
                (
                    Problem::MostlyFade,
                    "fades over more than half of its text's box",
                )
            } else if has_room_elsewhere(width) {
                (
                    Problem::NeedlessFade,
                    "fades here but not at the reference size",
                )
            } else {
                return None;
            };
            Some(Flag {
                problem,
                at: fade.clip,
                detail: detail.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Rect = Rect {
        left: 0.,
        top: 0.,
        right: 800.,
        bottom: 600.,
    };

    fn frame(ink: Vec<Ink>, boxes: Vec<PaintedBox>) -> Painted {
        Painted {
            width: WINDOW.right,
            height: WINDOW.bottom,
            ink,
            boxes,
        }
    }

    fn glyph(bounds: Rect, clip: Rect, order: u32) -> Ink {
        Ink {
            bounds,
            clip,
            order,
        }
    }

    fn solid(bounds: Rect, order: u32, alpha: f32) -> PaintedBox {
        PaintedBox {
            bounds,
            clip: WINDOW,
            order,
            fill: Fill::Solid(alpha),
        }
    }

    fn fade(bounds: Rect, clip: Rect, order: u32) -> PaintedBox {
        PaintedBox {
            bounds,
            clip,
            order,
            fill: Fill::Gradient,
        }
    }

    fn expected() -> Expected<'static> {
        Expected {
            zoom: 100,
            centered: false,
            reference_fades: &[],
        }
    }

    fn kinds(painted: &Painted, expected: Expected<'_>) -> Vec<Kind> {
        check(painted, expected)
            .into_iter()
            .map(|flag| flag.kind())
            .collect()
    }

    #[test]
    fn a_tidy_frame_has_no_flags() {
        let text = glyph(Rect::new(10., 10., 20., 20.), WINDOW, 2);
        let background = solid(WINDOW, 1, 1.);
        assert!(check(&frame(vec![text], vec![background]), expected()).is_empty());
    }

    #[test]
    fn ink_past_the_window_is_clipped() {
        let icon = glyph(Rect::new(10., 590., 30., 610.), WINDOW, 1);
        let flags = check(&frame(vec![icon], vec![]), expected());
        assert_eq!(flags[0].problem, Problem::PastWindow);
        assert_eq!(flags[0].at, Rect::new(10., 590., 30., 600.));
    }

    #[test]
    fn a_box_past_the_window_is_reported_once_with_what_it_holds() {
        let outer = solid(Rect::new(-100., 100., 900., 200.), 1, 1.);
        let inner = solid(Rect::new(700., 120., 850., 180.), 2, 1.);
        let flags = check(&frame(vec![], vec![outer, inner]), expected());
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].at, Rect::new(0., 100., 800., 200.));
    }

    #[test]
    fn text_cut_by_its_box_is_clipped_unless_it_fades() {
        let clip = Rect::new(10., 10., 100., 30.);
        let text = glyph(Rect::new(95., 12., 105., 28.), clip, 1);
        assert_eq!(
            kinds(&frame(vec![text], vec![]), expected()),
            [Kind::Clipped]
        );

        let over_end = fade(Rect::new(72., 10., 100., 30.), clip, 2);
        let faded = Expected {
            reference_fades: &[90.],
            ..expected()
        };
        assert!(kinds(&frame(vec![text], vec![over_end]), faded).is_empty());
    }

    #[test]
    fn stacked_lines_may_run_past_their_edge() {
        let first = Rect::new(300., 100., 800., 118.);
        let second = Rect::new(300., 118., 800., 136.);
        let ink = vec![
            glyph(Rect::new(795., 102., 805., 116.), first, 1),
            glyph(Rect::new(795., 120., 805., 134.), second, 2),
        ];
        assert!(check(&frame(ink, vec![]), expected()).is_empty());
    }

    #[test]
    fn a_scrolled_list_may_cut_its_last_row() {
        let list = Rect::new(10., 10., 200., 100.);
        let row = glyph(Rect::new(20., 95., 30., 110.), list, 1);
        assert!(check(&frame(vec![row], vec![]), expected()).is_empty());
    }

    #[test]
    fn text_over_text_overlaps_unless_a_box_hides_it() {
        let lower = glyph(Rect::new(10., 10., 30., 30.), WINDOW, 1);
        let upper = glyph(Rect::new(12., 12., 32., 32.), WINDOW, 3);
        let ink = vec![lower, upper];
        assert_eq!(
            kinds(&frame(ink.clone(), vec![]), expected()),
            [Kind::Overlap]
        );

        let popup = solid(Rect::new(0., 0., 100., 100.), 2, 1.);
        assert!(check(&frame(ink, vec![popup]), expected()).is_empty());
    }

    #[test]
    fn neighboring_glyphs_do_not_overlap() {
        let ink = vec![
            glyph(Rect::new(10., 10., 20., 30.), WINDOW, 1),
            glyph(Rect::new(19., 10., 29., 30.), WINDOW, 2),
            glyph(Rect::new(29., 25., 31., 30.), WINDOW, 3),
        ];
        assert!(check(&frame(ink, vec![]), expected()).is_empty());
    }

    #[test]
    fn a_group_over_the_scrim_must_be_centered() {
        let scrim = solid(WINDOW, 1, 0.5);
        let centered_expectation = Expected {
            centered: true,
            ..expected()
        };
        let middle = solid(Rect::new(300., 200., 500., 400.), 2, 1.);
        assert!(check(&frame(vec![], vec![scrim, middle]), centered_expectation).is_empty());

        let high = solid(Rect::new(300., 0., 500., 200.), 2, 1.);
        assert_eq!(
            kinds(&frame(vec![], vec![scrim, high]), centered_expectation),
            [Kind::OffCenter]
        );
    }

    #[test]
    fn a_whole_screen_group_is_centered_under_the_title_bar() {
        let bar = solid(Rect::new(0., 0., 800., 40.), 1, 1.);
        let logo = glyph(Rect::new(380., 300., 420., 340.), WINDOW, 2);
        let centered_expectation = Expected {
            centered: true,
            ..expected()
        };
        assert!(check(&frame(vec![logo], vec![bar]), centered_expectation).is_empty());
    }

    #[test]
    fn a_group_wider_than_the_window_is_still_centered() {
        let bar = solid(Rect::new(0., 0., 800., 40.), 1, 1.);
        let palette = solid(Rect::new(-100., 200., 900., 440.), 2, 1.);
        let hint = glyph(Rect::new(60., 420., 100., 436.), WINDOW, 3);
        let centered_expectation = Expected {
            centered: true,
            ..expected()
        };
        let flags = check(&frame(vec![hint], vec![bar, palette]), centered_expectation);
        assert!(!flags.iter().any(|flag| flag.problem == Problem::OffCenter));
    }

    #[test]
    fn docked_columns_can_squeeze_the_main_area() {
        let rail = solid(Rect::new(0., 40., 50., 600.), 1, 1.);
        let panel = solid(Rect::new(50., 40., 600., 600.), 2, 1.);
        let main = solid(Rect::new(600., 40., 800., 600.), 3, 1.);
        let flags = check(&frame(vec![], vec![rail, panel, main]), expected());
        assert_eq!(flags[0].problem, Problem::MainSqueezed);
        assert_eq!(flags[0].at, Rect::new(600., 0., 800., 600.));

        // The same 200 pixels are half as wide at 200% zoom.
        let narrow_panel = solid(Rect::new(50., 40., 300., 600.), 2, 1.);
        let frame = frame(vec![], vec![rail, narrow_panel]);
        assert!(check(&frame, expected()).is_empty());
        let zoomed = Expected {
            zoom: 200,
            ..expected()
        };
        assert_eq!(kinds(&frame, zoomed), [Kind::Squeezed]);
    }

    #[test]
    fn columns_wider_than_the_window_leave_no_main_area() {
        let columns = solid(Rect::new(0., 40., 900., 600.), 1, 1.);
        let flags = check(&frame(vec![], vec![columns]), expected());
        let squeezed = flags
            .iter()
            .find(|flag| flag.problem == Problem::MainSqueezed);
        assert!(squeezed.unwrap().detail.starts_with("main area is 0 px"));
    }

    #[test]
    fn a_floating_box_is_not_a_docked_column() {
        let rail = solid(Rect::new(0., 40., 50., 600.), 1, 1.);
        let palette = solid(Rect::new(40., 100., 760., 500.), 2, 1.);
        assert!(check(&frame(vec![], vec![rail, palette]), expected()).is_empty());
    }

    #[test]
    fn text_wrapping_after_every_letter_is_squeezed() {
        let letters: Vec<Ink> = (0..5)
            .map(|line| {
                let top = 100. + line as f32 * 20.;
                glyph(Rect::new(300., top, 308., top + 12.), WINDOW, line + 1)
            })
            .collect();
        let flags = check(&frame(letters.clone(), vec![]), expected());
        assert_eq!(flags[0].problem, Problem::LettersStacked);
        assert_eq!(flags[0].at, Rect::new(300., 100., 308., 192.));

        // Line numbers have their code after them.
        let mut numbered = letters;
        for line in 0..5 {
            let top = 100. + line as f32 * 20.;
            numbered.push(glyph(Rect::new(360., top, 368., top + 12.), WINDOW, 9));
            numbered.push(glyph(Rect::new(369., top, 377., top + 12.), WINDOW, 9));
        }
        assert!(check(&frame(numbered, vec![]), expected()).is_empty());
    }

    #[test]
    fn a_fade_is_flagged_when_its_box_fades_nowhere_else_or_is_mostly_fade() {
        let wide = Rect::new(10., 10., 200., 30.);
        let usual = fade(Rect::new(172., 10., 200., 30.), wide, 1);
        let at_reference = Expected {
            reference_fades: &[190.],
            ..expected()
        };
        assert!(check(&frame(vec![], vec![usual]), at_reference).is_empty());
        assert_eq!(
            kinds(&frame(vec![], vec![usual]), expected()),
            [Kind::Fading]
        );

        let tight = fade(
            Rect::new(10., 40., 38., 60.),
            Rect::new(10., 40., 40., 60.),
            1,
        );
        assert_eq!(
            kinds(&frame(vec![], vec![tight]), at_reference),
            [Kind::Fading]
        );
    }

    #[test]
    fn marking_outlines_a_flag_in_image_pixels() {
        let image = RgbaImage::from_pixel(100, 100, Rgba([0, 0, 0, 255]));
        let flag = Flag {
            problem: Problem::PastWindow,
            at: Rect::new(10., 10., 30., 30.),
            detail: String::new(),
        };
        let marked = marked(&image, &[flag]);
        assert_eq!(*marked.get_pixel(20, 20), MARK);
        assert_eq!(*marked.get_pixel(40, 40), Rgba([0, 0, 0, 255]));
        assert_eq!(*marked.get_pixel(5, 5), Rgba([0, 0, 0, 255]));
    }
}
