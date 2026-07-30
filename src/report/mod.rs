//! Decision-oriented report presentation models and renderers.

mod model;
mod render;

pub use model::{
    Badge, Confidence, DetailLevel, Finding, FindingGroup, Metric, NextStep, RenderOptions, Report,
    ScopeNote, Trend, Vocabulary,
};
