# AUR (archlinux user repository) Package

## Build

```bash
./mkpkgu
```

## Update

```bash
git clone ssh://aur@aur.archlinux.org/punktf.git
cp punktf/package/aur/PKGBUILD punktf/.
cp punktf/package/aur/build/.SRCINFO punktf/.

cd punktf
git add PKGBUILD .SRCINFO
git commit -m "MSG"
git push
```
