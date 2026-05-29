default: t

t:
    NO_DNA=1 anchor build
    cargo test --tests

tt:
    NO_DNA=1 anchor build
    ANCHOR_LITESVM_COLOR=1 cargo test --tests -- --nocapture --test-threads=1
