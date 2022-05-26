# archlinux-inputs-fsck

Lint a repository of PKGBUILDs to ensure all inputs are cryptographically pinned.

```sh
git clone https://github.com/kpcyrd/archlinux-inputs-fsck
cd archlinux-inputs-fsck
git clone --depth=1 https://github.com/archlinux/svntogit-community
cargo run --release -- check --all -W ./svntogit-community/
```

## License

GPLv3+
