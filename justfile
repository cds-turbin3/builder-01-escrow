default: t

t:
    NO_DNA=1 anchor build
    cargo test --tests

tt:
    NO_DNA=1 anchor build
    cargo test --tests -- --nocapture --test-threads=1
