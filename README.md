# archlinux-inputs-fsck

Lint a repository of PKGBUILDs to ensure all inputs are cryptographically pinned.

```sh
# Clone the archlinux-inputs-fsck source code
git clone https://github.com/kpcyrd/archlinux-inputs-fsck
cd archlinux-inputs-fsck
# Download the Arch Linux package repositories
git clone --depth=1 https://github.com/archlinux/svntogit-packages
git clone --depth=1 https://github.com/archlinux/svntogit-community
# Scan [core], [extra] and [community] for issues
cargo run --release -- check --all -W ./svntogit-packages/ -W ./svntogit-community/
```

## Generate TODO lists for specific issues

Use `-qq` to disable log output (except errors), `-r` to print package names to stdout, `-f git-commit-insecure-pin` to filter for a specific issue.

```sh
cargo run --release -- check --all -W ./svntogit-packages -W ./svntogit-community -qqrf git-commit-insecure-pin
```

You can use `-f` multiple times, to get a human readable report for specific issues do this:

```sh
cargo run --release -- check --all -W ./svntogit-packages -W ./svntogit-community -q -f git-commit-insecure-pin -f svn-insecure-pin
```

To get a list of all supported issue types do this:

```sh
% cargo run --release -- supported-issues
insecure-scheme
unknown-scheme
wrong-number-of-checksums
git-commit-insecure-pin
svn-insecure-pin
hg-revision-insecure-pin
bzr-insecure-pin
url-artifact-insecure-pin
```

## License

GPLv3+
