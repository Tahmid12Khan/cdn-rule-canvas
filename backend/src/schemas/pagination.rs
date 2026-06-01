//! Pagination DTOs (BACKEND CONTRACT §5).
//!
//! Query `?page=<1-based>&page_size=<n>`; `page_size` capped at 100, default 20.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Default page size when `page_size` is omitted.
pub const DEFAULT_PAGE_SIZE: u32 = 20;
/// Maximum allowed page size.
pub const MAX_PAGE_SIZE: u32 = 100;

/// Pagination query parameters.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PageParams {
    /// 1-based page number.
    #[serde(default)]
    pub page: Option<u32>,
    /// Page size (capped at [`MAX_PAGE_SIZE`]).
    #[serde(default)]
    pub page_size: Option<u32>,
}

impl Default for PageParams {
    fn default() -> Self {
        Self {
            page: None,
            page_size: None,
        }
    }
}

impl PageParams {
    /// Resolve raw params into SQL `(limit, offset)` plus the effective
    /// `(page, page_size)` for the response envelope.
    ///
    /// `page` defaults to 1 (and is clamped to ≥ 1). `page_size` defaults to
    /// [`DEFAULT_PAGE_SIZE`] and is clamped to `1..=`[`MAX_PAGE_SIZE`].
    pub fn resolve(&self) -> (i64, i64, u32, u32) {
        let page = self.page.unwrap_or(1).max(1);
        let page_size = self
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let limit = page_size as i64;
        let offset = ((page - 1) as i64) * limit;
        (limit, offset, page, page_size)
    }
}

/// Generic paginated response envelope.
#[derive(Debug, Serialize, ToSchema)]
#[aliases(
    PageFeatureRead = Page<crate::schemas::feature::FeatureRead>,
    PageVersionSummary = Page<crate::schemas::version::VersionSummary>
)]
pub struct Page<T> {
    /// Items on the current page.
    pub items: Vec<T>,
    /// 1-based page number.
    pub page: u32,
    /// Effective page size.
    pub page_size: u32,
    /// Total matching rows across all pages.
    pub total: i64,
}

impl<T> Page<T> {
    /// Build an envelope from items, the resolved page/size, and the total count.
    pub fn new(items: Vec<T>, page: u32, page_size: u32, total: i64) -> Self {
        Self {
            items,
            page,
            page_size,
            total,
        }
    }
}
