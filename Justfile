build:
    mkdir -p $OUTDIR
    rm -f output.log

    cargo run \
      -- \
      --log-level 'Debug' \
      --minifier-copy-on-failure \

serve:
    xdg-open http://localhost:3333/
    static-web-server
