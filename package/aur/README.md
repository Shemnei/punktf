# AUR (archlinux user repository) Package

## Build

```bash
./mkpkgu
```

## Update

### PKGBUILD

Update the fields `pkgver` and `sha512sums`.

The `sha512sums` must the be sha-512 sum of `Source code.tar.gz`.
To get the source run `mkpkgu` once.

```bash
shasum -a 512 ...
```

### Release

```bash
git clone ssh://aur@aur.archlinux.org/punktf.git
cp punktf/package/aur/PKGBUILD punktf/.
cp punktf/package/aur/build/.SRCINFO punktf/.

cd punktf
git add PKGBUILD .SRCINFO
git commit -m "MSG"
git push
```
