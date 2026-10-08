# Send to Wi-WWAV

The Shortcut that gets a notebook page from an iPhone or iPad into Learn's
Notes: take a photo, tap Share, tap **Send to Wi-WWAV**. It saves what was
shared into iCloud Drive, and Learn on the Mac reads it from there
(`docs/NOTES.md`, "The capture inbox"). There is no iOS app yet; this is the
whole of the phone's side.

- `Send to Wi-WWAV.shortcut` is the file to add. It is signed by Apple
  ("anyone" mode), which an iPhone asks for before it will add a Shortcut
  from a file.
- `Send to Wi-WWAV (unsigned).shortcut` is the same thing as a plain
  property list, for reading.
- `make_shortcut.py` writes both: `python3 tools/shortcut/make_shortcut.py`.
  Signing runs `shortcuts sign` on a Mac and needs the network.

## Adding it

1. Get the file onto the phone: AirDrop it, or put it in iCloud Drive and
   open it in Files.
2. Tap it, then **Add Shortcut**.
3. In Photos (or Files, or the camera roll), pick a photo, tap Share, and
   choose **Send to Wi-WWAV**. The first time, allow it to save to iCloud
   Drive.

It shows "Sent to Wi-WWAV" when the file is saved. Within a minute of the
Mac seeing it, the page is in Notes.

## What it does

For each photo or PDF shared:

1. **Format Date**: the current date as `2026-10-07 10.22.31`.
2. **Set Name**: `<that date> <the file's own name>`.
3. **Save File**: into `Shortcuts/Wi-WWAV Inbox` in iCloud Drive, without
   asking where.

Then one notification. Nothing else: no network call, nothing read from the
photo. Learn takes the time a photo was *taken* from the photo itself, and
falls back to the time in the name.

A Shortcut that is shared as a file can save without asking only inside its
own folder, `iCloud Drive/Shortcuts`. That is why the phone's inbox is
`Shortcuts/Wi-WWAV Inbox` and the Mac's own is `Wi-WWAV Inbox` at the top of
iCloud Drive; Learn watches both.

## If the file won't add

Make it by hand in the Shortcuts app; it is five actions.

1. New Shortcut, named **Send to Wi-WWAV**. In its details turn on **Show in
   Share Sheet**, and under Share Sheet Types keep Images, PDFs and Files.
2. **Repeat with Each** item in **Shortcut Input**.
3. Inside the repeat: **Format Date**, Current Date, Custom,
   `yyyy-MM-dd HH.mm.ss`.
4. **Set Name** of Repeat Item to `Formatted Date` `Repeat Item's Name`.
5. **Save File**: Renamed Item, Ask Where to Save off, path
   `/Wi-WWAV Inbox/`.
6. After End Repeat: **Show Notification**, "Sent to Wi-WWAV".

## What has been checked

The file is written and signed on a Mac, and Learn's side is tested: a file
named the way step 2 names it, put in the inbox folder, becomes a filed
note. The Shortcut itself has not been run on an iPhone by the people who
built this; if an action reads differently on your phone, the steps above
are the reference.
