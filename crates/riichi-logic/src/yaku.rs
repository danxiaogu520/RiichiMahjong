//! 役种公共类型。
//!
//! 役种判定由 [`crate::win_check::check_win`] 统一执行；本模块只保留
//! 役名的兼容导出，避免外部调用方依赖内部判定实现。

pub use crate::types::YakuName;
