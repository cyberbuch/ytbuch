//! Rate limiting middleware с tower_governor.
//! Общ лимит: ~100 заявки/минута на IP (burst 100, ~2/sec попълване).

use std::sync::Arc;

use governor::middleware::NoOpMiddleware;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::PeerIpKeyExtractor;
use tower_governor::GovernorLayer;

pub fn general_layer() -> GovernorLayer<PeerIpKeyExtractor, NoOpMiddleware, axum::body::Body> {
    let config = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(100)
        .finish()
        .expect("governor config");
    GovernorLayer::new(Arc::new(config))
}
