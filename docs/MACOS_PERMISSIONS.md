# macOS Full Disk Access Setup

## Why Full Disk Access is needed

Reclaim scans your entire disk to identify reclaimable space, including:
- Browser caches (Safari, Chrome, Firefox, etc.)
- System caches
- Application logs
- Build artifacts
- Virtual environments

Some of these directories are protected by macOS System Integrity Protection (SIP) and require **Full Disk Access** permission.

## How to grant Full Disk Access

### Option 1: Manual Setup (Recommended)

1. Open **System Settings** (or System Preferences on older macOS)
2. Go to **Privacy & Security** → **Full Disk Access**
3. Click the **🔒 lock icon** at the bottom left and authenticate
4. Click the **+** button
5. Navigate to and select the `reclaim-gui` binary:
   - If built with cargo: `~/repos/perso/reclaim/target/release/reclaim-gui`
   - If installed: `/usr/local/bin/reclaim-gui`
6. Enable the checkbox next to `reclaim-gui`
7. **Restart the application** for changes to take effect

### Option 2: Create macOS App Bundle

To properly request Full Disk Access, create an app bundle:

```bash
cd ~/repos/perso/reclaim
./create-macos-app.sh
```

This will create `Reclaim.app` with the proper Info.plist that requests Full Disk Access.

Then:
1. Move `Reclaim.app` to `/Applications/`
2. Launch it once (it will fail with permission errors)
3. Go to **System Settings** → **Privacy & Security** → **Full Disk Access**
4. Add `Reclaim.app` to the list
5. Relaunch the app

## What happens without Full Disk Access?

Without Full Disk Access, Reclaim will:
- ✅ Still scan accessible directories
- ⚠️ Skip protected directories (with warning messages)
- ⚠️ Show incomplete results for browser caches and system folders
- ✅ Continue functioning for all other directories

The app will NOT crash but results will be incomplete.

## Verification

After granting Full Disk Access, verify it's working:
1. Launch Reclaim
2. Check the scan completes without "Operation not permitted" errors
3. Verify browser caches are detected (Safari, Chrome, etc.)

## Security Note

Full Disk Access is a powerful permission. Reclaim:
- ✅ Only **reads** file metadata (size, dates)
- ❌ Does NOT read file contents
- ❌ Does NOT modify files without explicit user confirmation
- ✅ Is open source - you can review the code

## Troubleshooting

**"Operation not permitted" errors still appear:**
- Make sure you restarted the app after granting permission
- Try removing and re-adding the binary in Full Disk Access settings
- Check you added the correct binary path

**App not showing in Full Disk Access list:**
- You need to use the app bundle (Reclaim.app) not the raw binary
- Or manually navigate to the binary location using the + button

**Permission dialog doesn't appear:**
- macOS only shows permission dialogs for properly signed apps
- Use manual setup instead (see Option 1 above)
