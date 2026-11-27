"""Build Ubuntu 26.04 amd64 downloads without credentials, models or generated speech."""

import hashlib
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from pathlib import Path


def root_owner(info: tarfile.TarInfo) -> tarfile.TarInfo:
    info.uid = info.gid = 0
    info.uname = info.gname = "root"
    return info


def main() -> None:
    system = platform.freedesktop_os_release()
    if system["ID"] != "ubuntu" or system["VERSION_ID"] != "26.04" or platform.machine() != "x86_64":
        raise RuntimeError("Build these release downloads on Ubuntu 26.04 x86_64")
    root = Path(__file__).resolve().parents[1]
    with (root / "Cargo.toml").open("rb") as manifest:
        version = tomllib.load(manifest)["package"]["version"]
    subprocess.run(["cargo", "build", "--locked", "--release"], cwd=root, check=True)
    output = root / "dist"
    output.mkdir(exist_ok=True)
    deb = output / f"learn-swedish_{version}_ubuntu26.04_amd64.deb"
    archive = output / f"learn-swedish-{version}-ubuntu26.04-x86_64.tar.gz"
    with tempfile.TemporaryDirectory(prefix="learn-swedish-release-") as temporary:
        staging = Path(temporary)
        bundle = staging / f"learn-swedish-{version}-ubuntu26.04-x86_64"
        bundle.mkdir()
        binary = bundle / "learn-swedish"
        shutil.copyfile(root / "target/release/learn-swedish", binary)
        binary.chmod(0o755)
        for filename in ["README.md", "LICENSE"]:
            shutil.copyfile(root / filename, bundle / filename)
        story_files = sorted(
            path.name
            for path in (root / "stories").iterdir()
            if path.is_file() and path.suffix in {".xml", ".xsd"}
        )
        for folder, files in [
            ("scripts", ["requirements-azure.txt"]),
            ("stories", story_files),
            ("docs", ["story-xml-spec.md"]),
        ]:
            (bundle / folder).mkdir()
            for filename in files:
                shutil.copyfile(root / folder / filename, bundle / folder / filename)
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(bundle, arcname=bundle.name, filter=root_owner)

        package = staging / "debian"
        for directory in [
            "DEBIAN", "usr/bin", "usr/share/applications",
            "usr/share/doc/learn-swedish", "usr/share/learn-swedish/stories",
        ]:
            (package / directory).mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, package / "usr/bin/learn-swedish")
        shutil.copyfile(
            root / "packaging/io.github.hohum.LearnSwedish.desktop",
            package / "usr/share/applications/io.github.hohum.LearnSwedish.desktop",
        )
        shutil.copyfile(root / "LICENSE", package / "usr/share/doc/learn-swedish/copyright")
        shutil.copyfile(root / "README.md", package / "usr/share/doc/learn-swedish/README.md")
        shutil.copyfile(
            root / "docs/story-xml-spec.md",
            package / "usr/share/doc/learn-swedish/story-xml-spec.md",
        )
        shutil.copyfile(
            root / "scripts/requirements-azure.txt",
            package / "usr/share/learn-swedish/requirements-azure.txt",
        )
        for story_file in story_files:
            shutil.copyfile(
                root / "stories" / story_file,
                package / "usr/share/learn-swedish/stories" / story_file,
            )
        installed_bytes = binary.stat().st_size + sum(
            (root / "stories" / f).stat().st_size for f in story_files
        )
        (package / "DEBIAN/control").write_text(
            f"Package: learn-swedish\n"
            f"Version: {version}\n"
            "Architecture: amd64\n"
            "Maintainer: hohum <hohum@users.noreply.github.com>\n"
            "Section: education\n"
            "Priority: optional\n"
            f"Installed-Size: {(installed_bytes + 1023) // 1024 + 96}\n"
            "Depends: libc6 (>= 2.43), libgcc-s1, libgtk-4-1 (>= 4.22), "
            "libadwaita-1-0 (>= 1.9), libgstreamer1.0-0 (>= 1.28), "
            "gstreamer1.0-plugins-base, gstreamer1.0-plugins-good\n"
            "Recommends: python3-venv\n"
            "Homepage: https://github.com/hohum/learn_swedish\n"
            "Description: Swedish keyboard and pronunciation practice\n"
            " Native GNOME keyboard practice with bilingual follow-along text.\n"
            " Speech recordings are prepared separately and played offline.\n",
            encoding="utf-8",
        )
        subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(package), str(deb)], check=True)
    checksums = []
    for path in [deb, archive]:
        with path.open("rb") as artifact:
            digest = hashlib.file_digest(artifact, "sha256").hexdigest()
        checksums.append(f"{digest}  {path.name}\n")
        print(f"Built {path}")
    (output / "SHA256SUMS").write_text("".join(checksums), encoding="ascii")


if __name__ == "__main__":
    main()
