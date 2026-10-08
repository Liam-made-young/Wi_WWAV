#!/usr/bin/env python3
"""Writes "Send to Wi-WWAV.shortcut": the Shortcut that takes a photo or a
PDF from the share sheet on an iPhone or iPad and saves it in iCloud Drive,
where Learn on the Mac picks it up (docs/NOTES.md, "The capture inbox").

    python3 tools/shortcut/make_shortcut.py          # writes the unsigned file, then signs it
    python3 tools/shortcut/make_shortcut.py --plain  # the unsigned file only

What the Shortcut does, for each thing shared:
  1. Format Date: the current date, as 2026-10-07 10.22.31
  2. Set Name: "<that date> <its own name>"
  3. Save File: to Shortcuts/Wi-WWAV Inbox in iCloud Drive, without asking
and then one notification, "Sent to Wi-WWAV".

A Shortcut shared as a file can save without asking only inside its own
folder, iCloud Drive/Shortcuts, so that is where the inbox is. An iPhone
only adds a Shortcut file that Apple has signed: `shortcuts sign` does that
on a Mac, and needs the network.
"""

import plistlib
import subprocess
import sys
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
NAME = "Send to Wi-WWAV"
INBOX = "/Wi-WWAV Inbox/"
OBJECT = "￼"  # where a variable sits in a line of text


def attachment(value):
    return {"WFSerializationType": "WFTextTokenAttachment", "Value": value}


def text(*parts):
    """A line of text with variables in it: strings and variable dicts, in order."""
    string, ranges = "", {}
    for part in parts:
        if isinstance(part, str):
            string += part
        else:
            ranges["{%d, 1}" % len(string)] = part
            string += OBJECT
    return {
        "WFSerializationType": "WFTextTokenString",
        "Value": {"string": string, "attachmentsByRange": ranges},
    }


def output(of, name):
    return {"Type": "ActionOutput", "OutputUUID": of, "OutputName": name}


def action(identifier, **parameters):
    return {
        "WFWorkflowActionIdentifier": f"is.workflow.actions.{identifier}",
        "WFWorkflowActionParameters": parameters,
    }


def workflow():
    loop, stamp, named = (str(uuid.uuid4()).upper() for _ in range(3))
    item = {"Type": "Variable", "VariableName": "Repeat Item"}
    return {
        "WFWorkflowClientVersion": "2302.0.4",
        "WFWorkflowMinimumClientVersion": 900,
        "WFWorkflowMinimumClientVersionString": "900",
        "WFWorkflowIcon": {
            "WFWorkflowIconStartColor": 463140863,
            "WFWorkflowIconGlyphNumber": 59493,
        },
        "WFWorkflowImportQuestions": [],
        # In the share sheet, for images and PDFs.
        "WFWorkflowTypes": ["ActionExtension"],
        "WFWorkflowInputContentItemClasses": [
            "WFImageContentItem",
            "WFPDFContentItem",
            "WFGenericFileContentItem",
        ],
        "WFWorkflowOutputContentItemClasses": [],
        "WFWorkflowHasShortcutInputVariables": True,
        "WFWorkflowHasOutputFallback": False,
        "WFQuickActionSurfaces": [],
        "WFWorkflowNoInputBehavior": {
            "Name": "WFWorkflowNoInputBehaviorShowError",
            "Parameters": {
                "Error": "Share a photo or a PDF, then choose Send to Wi-WWAV."
            },
        },
        "WFWorkflowActions": [
            action(
                "repeat.each",
                GroupingIdentifier=loop,
                WFControlFlowMode=0,
                WFInput=attachment({"Type": "ExtensionInput"}),
            ),
            action(
                "format.date",
                UUID=stamp,
                WFDateFormatStyle="Custom",
                WFDateFormat="yyyy-MM-dd HH.mm.ss",
                WFDate=text({"Type": "CurrentDate"}),
            ),
            action(
                "setitemname",
                UUID=named,
                WFInput=attachment(item),
                WFName=text(
                    output(stamp, "Formatted Date"),
                    " ",
                    {
                        "Type": "Variable",
                        "VariableName": "Repeat Item",
                        "Aggrandizements": [
                            {
                                "Type": "WFPropertyVariableAggrandizement",
                                "PropertyName": "Name",
                            }
                        ],
                    },
                ),
                WFDontIncludeFileExtension=False,
            ),
            action(
                "documentpicker.save",
                WFInput=attachment(output(named, "Renamed Item")),
                WFAskWhereToSave=False,
                WFFileDestinationPath=INBOX,
                WFSaveFileOverwrite=False,
            ),
            action("repeat.each", GroupingIdentifier=loop, WFControlFlowMode=2),
            action(
                "notification",
                WFNotificationActionTitle="Wi-WWAV",
                WFNotificationActionBody="Sent to Wi-WWAV",
                WFNotificationActionSound=False,
            ),
        ],
    }


def main():
    unsigned = HERE / f"{NAME} (unsigned).shortcut"
    signed = HERE / f"{NAME}.shortcut"
    with open(unsigned, "wb") as f:
        plistlib.dump(workflow(), f, fmt=plistlib.FMT_BINARY)
    print(f"wrote {unsigned.name}")
    if "--plain" in sys.argv:
        return 0
    done = subprocess.run(
        ["shortcuts", "sign", "--mode", "anyone", "--input", str(unsigned), "--output", str(signed)],
        capture_output=True,
        text=True,
    )
    if done.returncode != 0 or not signed.exists():
        print("shortcuts sign failed:", (done.stderr or done.stdout).strip(), file=sys.stderr)
        return 1
    print(f"signed {signed.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
