use std::{
    collections::VecDeque,
    ops::Range,
    rc::{Rc, Weak},
};

use gpui::{Context, Task};
use language::{SyntaxCancellation, SyntaxState, TokenSpan, validate_syntax_ranges};
use text::{InvalidOffset, TextChange};

use super::{Document, DocumentSide, DocumentSnapshot};

const EDIT_HISTORY_LIMIT: usize = 128;

/// Identifies an editor's current attachment to a document.
#[derive(Clone, Default)]
pub struct SyntaxViewport(Rc<()>);

#[derive(Clone, Default)]
enum QueryOwner {
    #[default]
    Direct,
    Viewport(Weak<()>),
}

impl From<&SyntaxViewport> for QueryOwner {
    fn from(viewport: &SyntaxViewport) -> Self {
        Self::Viewport(Rc::downgrade(&viewport.0))
    }
}

impl PartialEq for QueryOwner {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Direct, Self::Direct) => true,
            (Self::Viewport(a), Self::Viewport(b)) => a.ptr_eq(b),
            _ => false,
        }
    }
}

impl QueryOwner {
    fn is_live(&self) -> bool {
        match self {
            Self::Direct => true,
            Self::Viewport(owner) => owner.strong_count() > 0,
        }
    }
}

#[derive(Default)]
pub(super) struct DocumentSyntax {
    base: Side,
    new: Side,
    changes: Vec<TextChange>,
}

#[derive(Default)]
struct Side {
    input: Option<DocumentSnapshot>,
    cache: Cache,
    worker: Option<Worker>,
    // Replacing a queued query moves it behind other owners already waiting.
    queries: VecDeque<Query>,
}

#[derive(Default)]
struct Query {
    owner: QueryOwner,
    pending: Option<Request>,
    result: Option<(Request, Vec<TokenSpan>)>,
}

#[derive(Default)]
struct Cache {
    input: Option<DocumentSnapshot>,
    syntax: SyntaxState,
}

#[derive(Clone)]
struct Request {
    input: DocumentSnapshot,
    ranges: Vec<Range<usize>>,
}

impl Side {
    fn query(&self, owner: &QueryOwner) -> Option<&Query> {
        self.queries.iter().find(|query| query.owner == *owner)
    }

    fn worker_for(&self, owner: &QueryOwner) -> Option<&Worker> {
        self.worker.as_ref().filter(|worker| worker.owner == *owner)
    }

    fn latest<'a>(&'a self, query: &'a Query) -> Option<&'a Request> {
        query
            .pending
            .as_ref()
            .or_else(|| {
                self.worker_for(&query.owner)
                    .filter(|worker| !worker.cancel.is_cancelled())
                    .map(|worker| &worker.request)
            })
            .or_else(|| query.result.as_ref().map(|(request, _)| request))
    }

    fn remove_closed_queries(&mut self) {
        self.queries.retain(|query| query.owner.is_live());
        if let Some(worker) = &self.worker
            && !worker.owner.is_live()
        {
            worker.cancel.cancel();
        }
    }
}

struct Worker {
    owner: QueryOwner,
    request: Request,
    cancel: SyntaxCancellation,
    _task: Task<()>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl DocumentSyntax {
    fn side(&self, side: DocumentSide) -> &Side {
        match side {
            DocumentSide::Base => &self.base,
            DocumentSide::New => &self.new,
        }
    }

    fn side_mut(&mut self, side: DocumentSide) -> &mut Side {
        match side {
            DocumentSide::Base => &mut self.base,
            DocumentSide::New => &mut self.new,
        }
    }
}

impl DocumentSnapshot {
    pub(super) fn same_context(&self, other: &Self) -> bool {
        self.source == other.source
            && self.present == other.present
            && self.language == other.language
    }
}

impl Document {
    /// Request syntax for a half-open UTF-8 byte range in one diff side. The
    /// latest standalone request on that side takes precedence. Completion notifies observers.
    pub fn request_syntax(
        &mut self,
        side: DocumentSide,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> Result<(), InvalidOffset> {
        self.request_syntax_ranges(side, vec![range], cx)
    }

    /// Request one batch of ordered, disjoint ranges on a side. Gaps need not
    /// be queried even though the parser retains full-file context.
    pub fn request_syntax_ranges(
        &mut self,
        side: DocumentSide,
        ranges: Vec<Range<usize>>,
        cx: &mut Context<Self>,
    ) -> Result<(), InvalidOffset> {
        self.request_syntax_for(QueryOwner::Direct, side, ranges, cx)
    }

    fn request_syntax_for(
        &mut self,
        owner: QueryOwner,
        side: DocumentSide,
        ranges: Vec<Range<usize>>,
        cx: &mut Context<Self>,
    ) -> Result<(), InvalidOffset> {
        let input = self.source_snapshot(side);
        validate_syntax_ranges(&input.text, &ranges)?;
        let state = self.syntax.side_mut(side);
        state.remove_closed_queries();
        let matches =
            |request: &Request| request.ranges == ranges && request.input.same_input(&input);
        if state
            .query(&owner)
            .and_then(|query| state.latest(query))
            .is_some_and(matches)
        {
            return Ok(());
        }
        if let Some(worker) = state.worker_for(&owner) {
            worker.cancel.cancel();
        }
        let mut query = state
            .queries
            .iter()
            .position(|query| query.owner == owner)
            .and_then(|index| state.queries.remove(index))
            .unwrap_or_else(|| Query {
                owner,
                ..Query::default()
            });
        query.pending = None;
        if matches!(query.owner, QueryOwner::Viewport(_)) && ranges.is_empty() {
            return Ok(());
        }
        // A scroll back can reuse the last completed window while the
        // superseded query drains.
        if query
            .result
            .as_ref()
            .is_some_and(|(request, _)| matches(request))
        {
            state.queries.push_back(query);
            return Ok(());
        }
        let plain =
            ranges.iter().all(Range::is_empty) || !input.present || input.language.syntax.is_none();
        state.input = Some(input.clone());
        let request = Request { input, ranges };
        if plain {
            query.result = Some((request, Vec::new()));
            cx.notify();
        } else {
            query.pending = Some(request);
        }
        state.queries.push_back(query);
        self.start_syntax(side, cx);
        Ok(())
    }

    /// Tokens for the requested range of the current input. Returns `None`
    /// while work is pending or stale, and an empty slice for plain text.
    pub fn syntax_tokens(&self, side: DocumentSide, range: Range<usize>) -> Option<&[TokenSpan]> {
        let state = self.syntax.side(side);
        let query = state.query(&QueryOwner::Direct)?;
        let (request, spans) = query.result.as_ref()?;
        (std::ptr::eq(state.latest(query)?, request)
            && request.ranges == [range]
            && request.input.same_input(&self.source_snapshot(side)))
        .then_some(spans.as_slice())
    }

    /// Whether this viewport needs to replace its padded coverage.
    pub fn syntax_viewport_needs(
        &self,
        viewport: &SyntaxViewport,
        side: DocumentSide,
        visible: &[Range<usize>],
    ) -> bool {
        let state = self.syntax.side(side);
        let previous = state
            .query(&viewport.into())
            .and_then(|query| state.latest(query));
        if visible.is_empty() {
            return previous.is_some();
        }
        let input = self.source_snapshot(side);
        if !input.present || input.language.syntax.is_none() {
            return false;
        }
        let Some(previous) = previous else {
            return true;
        };
        let mut ranges = previous.ranges.iter().peekable();
        visible.iter().any(|visible| {
            while ranges
                .peek()
                .is_some_and(|range| range.end <= visible.start)
            {
                ranges.next();
            }
            ranges
                .peek()
                .is_none_or(|range| visible.start < range.start || visible.end > range.end)
        })
    }

    /// Each viewport has its own query budget and shares this side's parser.
    pub fn request_viewport_syntax(
        &mut self,
        viewport: &SyntaxViewport,
        side: DocumentSide,
        ranges: Vec<Range<usize>>,
        cx: &mut Context<Self>,
    ) -> Result<(), InvalidOffset> {
        self.request_syntax_for(viewport.into(), side, ranges, cx)
    }

    /// Discard a detached editor's queued work, running query, and spans.
    pub fn release_syntax_viewport(&mut self, viewport: &SyntaxViewport) {
        let owner = QueryOwner::from(viewport);
        for state in [&mut self.syntax.base, &mut self.syntax.new] {
            state.queries.retain(|query| query.owner != owner);
            if let Some(worker) = state.worker_for(&owner) {
                worker.cancel.cancel();
            }
        }
    }

    /// Published spans for this viewport, including its last result while a
    /// replacement query runs. Input mutations clear stale spans.
    pub fn syntax_spans(&self, viewport: &SyntaxViewport, side: DocumentSide) -> &[TokenSpan] {
        self.syntax
            .side(side)
            .query(&viewport.into())
            .and_then(|query| query.result.as_ref())
            .map_or(&[], |(_, spans)| spans)
    }

    pub(super) fn record_syntax_change(&mut self, change: &TextChange) {
        let changes = &mut self.syntax.changes;
        if change.edits.len() > EDIT_HISTORY_LIMIT {
            changes.clear();
            return;
        }
        let mut remaining = EDIT_HISTORY_LIMIT - change.edits.len();
        let keep = changes
            .iter()
            .rev()
            .take_while(|old| {
                if let Some(next) = remaining.checked_sub(old.edits.len()) {
                    remaining = next;
                    true
                } else {
                    false
                }
            })
            .count();
        changes.drain(..changes.len() - keep);
        changes.push(change.clone());
    }

    pub(super) fn invalidate_syntax(&mut self, side: DocumentSide) {
        let input = self.source_snapshot(side);
        let state = self.syntax.side_mut(side);
        if state
            .input
            .as_ref()
            .is_some_and(|previous| !previous.same_input(&input))
        {
            state.input = None;
            state.queries.clear();
            if let Some(worker) = &state.worker {
                worker.cancel.cancel();
            }
        }
    }

    fn start_syntax(&mut self, side: DocumentSide, cx: &mut Context<Self>) {
        let state = self.syntax.side_mut(side);
        state.remove_closed_queries();
        if state.worker.is_some() {
            return;
        }
        let Some((owner, request)) = state.queries.iter_mut().find_map(|query| {
            query
                .pending
                .take()
                .map(|request| (query.owner.clone(), request))
        }) else {
            return;
        };
        let mut cache = std::mem::take(&mut state.cache);
        let changes = match side {
            DocumentSide::Base => Vec::new(),
            DocumentSide::New => self.syntax.changes.clone(),
        };
        let cancel = SyntaxCancellation::default();
        let work = request.clone();
        let work_cancel = cancel.clone();
        let task = cx.spawn(async move |this, cx| {
            let (cache, spans) = cx
                .background_executor()
                .spawn(async move {
                    let Request { input, ranges } = work;
                    if cache
                        .input
                        .as_ref()
                        .is_none_or(|old| !old.same_context(&input))
                    {
                        cache.syntax = SyntaxState::default();
                    }
                    let spans = cache
                        .syntax
                        .tokens_for_ranges(
                            &input.text,
                            &input.language,
                            &changes,
                            &ranges,
                            &work_cancel,
                        )
                        .expect("request range validated against its snapshot");
                    cache.input = Some(input);
                    (cache, spans)
                })
                .await;
            this.update(cx, |document, cx| {
                let current = document.source_snapshot(side);
                let state = document.syntax.side_mut(side);
                state.cache = cache;
                if let Some(worker) = state.worker.take()
                    && !worker.cancel.is_cancelled()
                    && worker.request.input.same_input(&current)
                    && worker.owner.is_live()
                    && let Some(query) = state
                        .queries
                        .iter_mut()
                        .find(|query| query.owner == worker.owner)
                {
                    query.result = Some((worker.request.clone(), spans));
                    cx.notify();
                }
                document.start_syntax(side, cx);
            })
            .ok();
        });
        self.syntax.side_mut(side).worker = Some(Worker {
            owner,
            request,
            cancel,
            _task: task,
        });
    }
}
