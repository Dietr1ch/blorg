build:
    rm -f output.log
    cargo run \
      -- \
      --title "FIXME" \
      --description "FIXME" \
      --root-address "https://please.fix.me"
      --log-level 'Debug' \
      --minifier-copy-on-failure \

serve:
    xdg-open http://localhost:3333/
    static-web-server
