const REPO = "elci-group/padagonia";
const RELEASES_URL = `https://api.github.com/repos/${REPO}/releases`;
const FALLBACK_RELEASES = [
  {
    name: "v0.1.2",
    tag_name: "v0.1.2",
    published_at: "2026-07-08T17:06:00Z",
    html_url: "https://github.com/elci-group/padagonia/releases/tag/v0.1.2",
    body: "Logo, enterprise-grade CLI help, ASCII art, and version bump to v0.1.2.",
    assets: [],
    zipball_url: "https://github.com/elci-group/padagonia/archive/refs/tags/v0.1.2.zip",
    tarball_url: "https://github.com/elci-group/padagonia/archive/refs/tags/v0.1.2.tar.gz",
    prerelease: false,
    draft: false,
  },
  {
    name: "v0.1.1",
    tag_name: "v0.1.1",
    published_at: "2026-07-08T13:46:00Z",
    html_url: "https://github.com/elci-group/padagonia/releases/tag/v0.1.1",
    body: "HNSW vector search integration and deterministic benchmarks.",
    assets: [],
    zipball_url: "https://github.com/elci-group/padagonia/archive/refs/tags/v0.1.1.zip",
    tarball_url: "https://github.com/elci-group/padagonia/archive/refs/tags/v0.1.1.tar.gz",
    prerelease: false,
    draft: false,
  },
];

function formatDate(iso) {
  const d = new Date(iso);
  return d.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

function renderReleases(releases) {
  const container = document.getElementById("releases");
  const status = document.getElementById("release-status");
  container.innerHTML = "";

  if (!Array.isArray(releases) || releases.length === 0) {
    status.textContent = "No releases found.";
    return;
  }

  status.style.display = "none";

  releases.forEach((rel, index) => {
    const isLatest = index === 0 && !rel.prerelease && !rel.draft;
    const item = document.createElement("article");
    item.className = `release-item ${isLatest ? "latest" : ""}`;

    const title = document.createElement("div");
    title.innerHTML = `<strong>${rel.name || rel.tag_name}</strong>${isLatest ? '<span class="release-badge">Latest</span>' : ""}`;

    const meta = document.createElement("div");
    meta.className = "release-meta";
    meta.textContent = `Released on ${formatDate(rel.published_at)}${rel.prerelease ? " · Pre-release" : ""}`;

    const notes = document.createElement("p");
    notes.textContent = rel.body ? rel.body.split("\n")[0] : "No release notes provided.";

    const left = document.createElement("div");
    left.style.flex = "1 1 260px";
    left.appendChild(title);
    left.appendChild(meta);
    left.appendChild(notes);

    const links = document.createElement("div");
    links.style.display = "flex";
    links.style.flexWrap = "wrap";
    links.style.gap = "0.5rem";
    links.style.alignItems = "center";

    const releaseLink = document.createElement("a");
    releaseLink.className = "btn btn-primary";
    releaseLink.href = rel.html_url;
    releaseLink.textContent = "Release notes";

    const sourceLink = document.createElement("a");
    sourceLink.className = "btn btn-secondary";
    sourceLink.href = rel.zipball_url;
    sourceLink.textContent = "Source (.zip)";

    links.appendChild(releaseLink);
    links.appendChild(sourceLink);

    item.appendChild(left);
    item.appendChild(links);
    container.appendChild(item);
  });
}

async function loadReleases() {
  try {
    const response = await fetch(RELEASES_URL);
    if (!response.ok) throw new Error(`GitHub API returned ${response.status}`);
    const releases = await response.json();
    // Filter out drafts unless authenticated; public API hides drafts anyway.
    const visible = releases.filter((r) => !r.draft);
    renderReleases(visible);
  } catch (err) {
    console.warn("Could not fetch live releases, showing fallback list.", err);
    document.getElementById("release-status").textContent =
      "Could not reach the GitHub API. Showing the most recent known releases.";
    renderReleases(FALLBACK_RELEASES);
  }
}

loadReleases();
