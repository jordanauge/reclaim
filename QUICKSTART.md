# Reclaim - Quick Start Guide

Get started with Reclaim in 5 minutes.

## Installation

### macOS

**Download and install:**
```bash
# Apple Silicon (M1/M2/M3)
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/Reclaim-macos-silicon.dmg
open Reclaim-macos-silicon.dmg

# Intel
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/Reclaim-macos-intel.dmg
open Reclaim-macos-intel.dmg
```

**Grant permissions:**
1. Drag Reclaim.app to Applications
2. Open Reclaim.app
3. System Settings → Privacy & Security → Full Disk Access
4. Add Reclaim.app
5. Restart Reclaim

### Linux

**AppImage (recommended):**
```bash
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/Reclaim-linux-x86_64.AppImage
chmod +x Reclaim-linux-x86_64.AppImage
./Reclaim-linux-x86_64.AppImage
```

**Debian/Ubuntu:**
```bash
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/reclaim_amd64.deb
sudo dpkg -i reclaim_amd64.deb
reclaim
```

### Windows

```bash
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/Reclaim-windows-portable.zip
unzip Reclaim-windows-portable.zip
.\Reclaim.exe
```

## First Run

### 1. Launch Reclaim

The app opens with a clean interface showing three main sections:
- **Top**: Scan controls and selected size
- **Center**: Results table/treemap/disk overview
- **Bottom**: Status bar with progress

### 2. Start a Scan

Click **"Start Scan"** button. Reclaim will:
1. Load cached results (instant, if available)
2. Verify cached items (1-5 seconds)
3. Scan for new items (10-60 seconds)
4. Analyze disk space (optional, 30-120 seconds)

**First scan** takes longer, subsequent scans use cache.

### 3. Review Results

Results appear in **Table view** by default, grouped intelligently:

**Group Types:**
- 🔁 **Duplicates**: Same file name and size
- 📄 **Similar**: Similar naming patterns
- 📁 **Same Directory**: Items in same folder
- 📂 **Common Ancestor**: Related by path

Click **▶** to expand groups and see individual items.

### 4. Understand the Columns

| Column | Description |
|--------|-------------|
| **Select** | Checkbox to mark for deletion |
| **Name/Group** | File or group name |
| **Kind** | Artifact type (venv, node_modules, etc.) |
| **Cache** | Freshness (✓ cached, ~ estimated, ⊙ new) |
| **Size** | Disk space used |
| **Score** | Cleanup priority (0-100) |
| **Location** | Parent folder |
| **Action** | What will happen (Delete, Prune, etc.) |

### 5. Select Items

**Individual items:**
- Check boxes next to items you want to remove
- Or use filters to narrow down (see below)

**Entire groups:**
- Click group checkbox to select all items in group

**Selected size** appears in top bar.

### 6. Preview Before Cleaning

Click **"Preview Cleanup"** to see what will happen:
- Shows list of items
- Total space to reclaim
- **No files are modified**

### 7. Clean

Click **"Clean Selected"** and confirm.

Reclaim will:
- Delete/prune selected items
- Show progress
- Update cache
- Display results

## Using Filters

### By Type

Click **"Filters"** and check/uncheck artifact types:
- ☑ Python venv
- ☑ Node.js modules
- ☑ Build directories
- ☑ Docker caches
- etc.

### By Size

Adjust sliders:
- **Min size**: Skip small files
- **Max size**: Focus on large items

### By Age

Filter by days old:
- **Min age**: Recent files
- **Max age**: Old files

### By Score

Set minimum cleanup score (0-100):
- Higher score = safer to remove
- Lower score = more caution needed

## View Modes

### Table View (Default)

Grouped, sortable list of candidates.

**Sort by:**
- Size (largest first)
- Score (best candidates first)
- Age (oldest first)
- Kind (group by type)

### Treemap View

Visual, area-proportional rectangles:
- **Larger rectangle** = more disk space
- **Colors** indicate group type
- **Hover** for details
- **Click** to drill down

### Disk Overview

Full disk categorization:
- **Pie chart** shows 6 main categories
- **Categories**: System, Media, Documents, Code, Reclaimable, Other
- **Click** category to explore

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Cmd/Ctrl + R` | Start scan |
| `Cmd/Ctrl + D` | Preview cleanup |
| `Cmd/Ctrl + E` | Export to JSON |
| `Cmd/Ctrl + ,` | Settings |
| `Escape` | Close dialogs |

## Common Workflows

### Clean Up Development Projects

1. Start scan
2. Filter by:
   - ☑ Python venv
   - ☑ Rust target/
   - ☑ Node.js node_modules
3. Sort by **Size**
4. Select large, old items
5. Clean

**Expected savings**: 10-50 GB on active dev machines

### Remove Docker Cruft

1. Start scan
2. Filter by:
   - ☑ Docker Build Caches
3. Sort by **Age**
4. Select items older than 30 days
5. Clean

### Free Up Space Quickly

1. Start scan
2. Go to **Disk Overview**
3. Click **Reclaimable** category
4. Review items
5. Select obvious candidates
6. Clean

## Settings

Click **⚙ Settings** (top right):

**Permissions:**
- Check Full Disk Access status (macOS)
- Grant access if needed

**View:**
- ☑ Show groups by default
- Toggles grouped vs flat view

**Updates:**
- Shows install method (Standalone/System Package)
- Check for updates (if standalone)
- Or shows package manager command

## Tips & Tricks

### Speed Up Scans

- **Auto-scan**: Enable in settings to scan on startup
- **Cache**: Subsequent scans use cache (much faster)
- **Skip disk analysis**: Disable in settings if you don't need category breakdown

### Avoid Mistakes

1. **Always preview first**: Use "Preview Cleanup"
2. **Check the Action column**: See what will happen
3. **Start small**: Clean a few items first
4. **Verify**: Check your projects still work

### Explore Genealogy

Click **🔍** next to a group to see:
- Parent folder
- Sibling folders
- Total size in parent

Useful for understanding context.

### Export Results

Click **"Export"** to save scan results as JSON:
- Share with team
- Track over time
- Analyze externally

## Troubleshooting

### macOS: "App is damaged"

**Solution:**
```bash
xattr -cr /Applications/Reclaim.app
```

Or right-click → Open (first time).

### Linux: AppImage won't run

**Solution:**
```bash
sudo apt install fuse libfuse2
```

### Scan finds nothing

**Check:**
1. Full Disk Access granted (macOS)
2. Running from home directory
3. Filters not too restrictive

### App crashes during scan

**Try:**
1. Disable disk analysis in settings
2. Reduce scan scope
3. Check logs: `~/.cache/reclaim/`

## Next Steps

- Read [README.md](README.md) for full feature list
- See [DISTRIBUTION.md](DISTRIBUTION.md) for packaging info
- Check [DEVELOPMENT.md](DEVELOPMENT.md) to contribute
- Browse [docs/](docs/) for technical details

## Getting Help

- **Issues**: https://github.com/jordanauge/reclaim/issues
- **Discussions**: https://github.com/jordanauge/reclaim/discussions
- **Email**: [Your email if you want to provide one]

---

**Made with 🦀 Rust**
