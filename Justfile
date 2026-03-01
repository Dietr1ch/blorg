build:
    mkdir -p $OUTDIR
    rm -f output.log

    cargo run \
      -- \
      --log-level 'Debug' \
      --minify-html \
      --minifier-copy-on-failure

fmt:
      cargo fmt

clippy:
    cargo clippy \
      --all-targets \
      --all-features \
      --allow-dirty \
      --fix

test:
    cargo test  --all-targets --no-fail-fast


serve:
    xdg-open http://localhost:3003/ || echo "Go to http://localhost:3003/"
    static-web-server
