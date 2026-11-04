pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// First-run tour, after upstream WelcomeView.swift: hello, drag chord setup,
// external tools, done. Closing it any way marks it done (App.finishWelcome);
// it stays reachable through App.showWelcome(). Commands are only shown for
// copying; Nashi never runs privileged commands itself.
ApplicationWindow {
    id: root

    readonly property int pageCount: 4
    property int page: 0
    readonly property var names: [qsTr("Hello"), qsTr("Drag chords"), qsTr("Tools"), qsTr("Ready")]

    // Chord readiness, from SetupCheck (input group) and nashi-inputd on D-Bus.
    readonly property int groupState: App.setup.groupState
    readonly property bool chordsReady: App.setup.inInputGroup && App.inputd.available
    readonly property bool toolsReady: !App.deps.loading && App.deps.missingCount === 0 && App.deps.error.length === 0

    function reset() { page = 0 }
    function next() { if (page < pageCount - 1) page += 1 }
    function back() { if (page > 0) page -= 1 }
    function recheck() {
        App.setup.refresh()
        App.inputd.refresh()
    }

    visible: false
    width: 800
    height: 560
    minimumWidth: 720
    minimumHeight: 520
    title: qsTr("Welcome to Nashi")
    color: Theme.canvas

    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.surface
    palette.text: Theme.text
    palette.button: Theme.surface
    palette.buttonText: Theme.text
    palette.highlight: Theme.gold
    palette.highlightedText: Theme.buttonInk

    onClosing: App.finishWelcome()
    // Fixing the setup happens in a terminal; look again when the user is back.
    onActiveChanged: if (active) recheck()
    onPageChanged: {
        if (page === 1)
            recheck()
        else if (page === 2 && !App.deps.loading)
            App.deps.refresh()
    }

    Shortcut { sequence: "Escape"; onActivated: root.close() }
    Shortcut { sequences: ["Right", "Return", "Enter"]; onActivated: root.page === root.pageCount - 1 ? root.close() : root.next() }
    Shortcut { sequence: "Left"; onActivated: root.back() }

    // A shell command shown in a monospace well with a copy button.
    component CommandRow: Rectangle {
        id: cmd
        property string command: ""
        Layout.fillWidth: true
        implicitHeight: cmdRow.implicitHeight + 16
        radius: Theme.radiusCompact
        color: Theme.inset
        border.width: 1
        border.color: Theme.border

        RowLayout {
            id: cmdRow
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 6
            spacing: 8
            TextEdit {
                Layout.fillWidth: true
                text: cmd.command
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.WrapAnywhere
                font.family: "monospace"
                font.pixelSize: Theme.sizeSmall + 1
                color: Theme.text
                selectionColor: Theme.gold
                selectedTextColor: Theme.buttonInk
            }
            NButton {
                compact: true
                kind: "quiet"
                iconName: "edit-copy"
                text: qsTr("Copy")
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Copy to the clipboard")
                onClicked: App.actions.copyText(cmd.command)
            }
        }
    }

    // "Label ........ [state]" line of the chord check.
    component StatusRow: RowLayout {
        id: status
        property string label: ""
        property string stateText: ""
        property bool ok: false
        property bool warn: false
        Layout.fillWidth: true
        spacing: 8
        Text {
            Layout.fillWidth: true
            text: status.label
            elide: Text.ElideRight
            font.pixelSize: Theme.sizeBody
            font.weight: Font.Medium
            color: Theme.text
        }
        Pill {
            text: status.stateText
            tint: status.ok ? Theme.success : status.warn ? Theme.goldInk : Theme.danger
        }
    }

    component Body: Text {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font.pixelSize: Theme.sizeBody
        lineHeight: 1.15
        color: Theme.secondaryText
    }

    component Note: Text {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font.pixelSize: Theme.sizeSmall
        color: Theme.secondaryText
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Top bar
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 22
            Layout.rightMargin: 22
            Layout.topMargin: 15
            Item { Layout.preferredWidth: skip.implicitWidth }
            Text {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                text: qsTr("A FRESH START")
                font.pixelSize: Theme.sizeTiny
                font.weight: Font.Bold
                font.letterSpacing: 1.5
                color: Theme.secondaryText
            }
            NButton {
                id: skip
                compact: true
                kind: "quiet"
                text: qsTr("Skip tour")
                // Hidden on the last page without shifting the centred title.
                opacity: root.page < root.pageCount - 1 ? 1 : 0
                enabled: opacity > 0
                onClicked: root.close()
            }
        }

        // Art + copy
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 36
            Layout.rightMargin: 36
            spacing: 32

            Item {
                Layout.preferredWidth: 280
                Layout.preferredHeight: 300
                Layout.alignment: Qt.AlignVCenter

                Rectangle {
                    id: circle
                    anchors.centerIn: parent
                    width: 260
                    height: 260
                    radius: width / 2
                    color: root.page === 1 || root.page === 2 ? Theme.sage : Theme.sand
                    Behavior on color { ColorAnimation { duration: Theme.normal } }
                }

                NashiMark {
                    anchors.centerIn: circle
                    visible: root.page === 0 || root.page === 3
                    size: 160
                    happy: root.page === 3
                    rotation: root.page === 3 ? -6 : 0
                    Behavior on rotation { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutCubic } }
                }

                // Chord keys: the three modes of the wheel.
                Column {
                    anchors.centerIn: circle
                    visible: root.page === 1
                    spacing: 12
                    scale: 1.45
                    KeyChip { anchors.horizontalCenter: parent.horizontalCenter; keyText: "⇧"; label: qsTr("Convert") }
                    KeyChip { anchors.horizontalCenter: parent.horizontalCenter; keyText: "Ctrl"; label: qsTr("Tools") }
                    KeyChip { anchors.horizontalCenter: parent.horizontalCenter; keyText: "Alt"; label: qsTr("Actions") }
                }

                Column {
                    anchors.centerIn: circle
                    visible: root.page === 2
                    spacing: 10
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: App.deps.loading ? "…" : App.deps.missingCount === 0 ? "✓" : App.deps.missingCount
                        font.pixelSize: 84
                        font.weight: Font.Bold
                        color: root.toolsReady ? Theme.forest : Theme.goldInk
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: App.deps.loading ? qsTr("checking")
                              : App.deps.missingCount === 0 ? qsTr("all tools found")
                              : qsTr("missing")
                        font.pixelSize: Theme.sizeSmall
                        font.weight: Font.DemiBold
                        font.letterSpacing: 1
                        color: Theme.forest
                    }
                }
            }

            // Copy column; scrolls if a long install command needs the room.
            Flickable {
                id: copyFlick
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.topMargin: 12
                Layout.bottomMargin: 12
                contentWidth: width
                contentHeight: Math.max(copy.implicitHeight, height)
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: copy.implicitHeight > copyFlick.height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff }

                ColumnLayout {
                    id: copy
                    width: copyFlick.width - 8
                    y: Math.max(0, (copyFlick.height - implicitHeight) / 2)
                    spacing: 14

                    Pill {
                        text: root.names[root.page].toUpperCase()
                        tint: root.page === 3 ? Theme.forest : Theme.goldInk
                    }

                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        font.pixelSize: Theme.sizeHero + 2
                        font.weight: Font.Bold
                        font.letterSpacing: -1
                        lineHeight: 0.95
                        color: Theme.text
                        text: [
                            qsTr("Nice files.\nNo fuss."),
                            root.chordsReady ? qsTr("Drag chords\nare ready.") : qsTr("Drag chords.\nOne-time setup."),
                            root.toolsReady ? qsTr("Your tools\nare on board.") : qsTr("A few tools\nto go."),
                            qsTr("You’re all set.\nMake something good.")
                        ][root.page]
                    }

                    // 1 · Hello
                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: root.page === 0
                        spacing: 12
                        Body {
                            text: qsTr("Drag a file from Dolphin or your desktop, hold Shift, Ctrl or Alt, and drop it onto the wheel that appears. Pick a new format, a tool or an action.")
                        }
                        Body {
                            text: qsTr("Your files are never changed. Nashi saves a fresh copy next to the original.")
                        }
                        Pill {
                            iconName: "object-locked"
                            text: qsTr("Everything stays on this computer")
                        }
                    }

                    // 2 · Drag chords
                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: root.page === 1
                        spacing: 10
                        Body {
                            text: qsTr("A small background service, nashi-inputd, notices when you hold a key while dragging. It needs to read your input devices.")
                        }
                        StatusRow {
                            label: qsTr("Input device access")
                            ok: root.groupState === SetupCheck.InGroup
                            warn: root.groupState === SetupCheck.NeedsRelogin
                            stateText: root.groupState === SetupCheck.InGroup ? qsTr("Ready")
                                       : root.groupState === SetupCheck.NeedsRelogin ? qsTr("Log out and back in")
                                       : root.groupState === SetupCheck.NoGroup ? qsTr("No input group")
                                       : qsTr("Not set up")
                        }
                        StatusRow {
                            label: qsTr("Chord service")
                            ok: App.inputd.available
                            stateText: App.inputd.available
                                       ? (App.inputd.enabled ? qsTr("Running") : qsTr("Paused"))
                                       : qsTr("Not running")
                            warn: App.inputd.available && !App.inputd.enabled
                        }

                        Note {
                            Layout.topMargin: 4
                            visible: root.groupState === SetupCheck.NotInGroup
                            text: qsTr("Add yourself to the input group, then log out and back in:")
                        }
                        CommandRow {
                            visible: root.groupState === SetupCheck.NotInGroup
                            command: App.setup.groupCommand
                        }
                        Note {
                            visible: root.groupState === SetupCheck.NeedsRelogin
                            text: qsTr("You are in the input group, but this session started before that. Log out and back in to finish.")
                        }
                        Note {
                            visible: root.groupState === SetupCheck.NoGroup
                            text: qsTr("This system has no input group. Make sure /dev/input/event* is readable by your user, for example with the udev rule shipped with Nashi.")
                        }
                        Note {
                            Layout.topMargin: 4
                            visible: !App.inputd.available
                            text: qsTr("Start the service now and with every login:")
                        }
                        CommandRow {
                            visible: !App.inputd.available
                            command: App.setup.serviceCommand
                        }
                        Note {
                            visible: root.chordsReady
                            text: qsTr("Hold a key while dragging a file and the wheel appears.")
                        }
                        Note {
                            color: Theme.tertiaryText
                            text: qsTr("Nashi never runs these for you. Programs in the input group can read every input device; nashi-inputd only looks at the mouse button and Shift, Ctrl and Alt, and never records keys.")
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            NButton {
                                compact: true
                                iconName: "view-refresh"
                                text: qsTr("Check again")
                                onClicked: root.recheck()
                            }
                            Note {
                                text: qsTr("You can also use Nashi without chords: drop files on its window.")
                            }
                        }
                    }

                    // 3 · Tools
                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: root.page === 2
                        spacing: 10
                        Body {
                            text: qsTr("Nashi uses the programs already on your system, like FFmpeg and ImageMagick. Nothing is downloaded.")
                        }
                        Body {
                            visible: App.deps.loading
                            text: qsTr("Checking your tools…")
                        }
                        Body {
                            visible: App.deps.error.length > 0
                            color: Theme.danger
                            text: App.deps.error
                        }
                        Body {
                            visible: !App.deps.loading && App.deps.error.length === 0 && App.deps.missingCount === 0
                            text: qsTr("Everything Nashi can use is installed.")
                        }
                        Note {
                            visible: !App.deps.loading && App.deps.missingCount > 0
                            text: qsTr("%n tool(s) missing. Items that need them stay hidden until you install them:", "", App.deps.missingCount)
                        }
                        CommandRow {
                            visible: !App.deps.loading && App.deps.missingCount > 0 && App.deps.installCommand.length > 0
                            command: App.deps.installCommand
                        }
                        Note {
                            color: Theme.tertiaryText
                            visible: !App.deps.loading && App.deps.missingCount > 0
                            text: qsTr("Optional. You can see every tool later under Tools in the main window.")
                        }
                        NButton {
                            compact: true
                            iconName: "view-refresh"
                            text: qsTr("Check again")
                            enabled: !App.deps.loading
                            onClicked: App.deps.refresh()
                        }
                    }

                    // 4 · Done
                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: root.page === 3
                        spacing: 12
                        Body {
                            text: qsTr("Nashi waits in the system tray. Drag a file with a key held, or open the main window to pick files.")
                        }
                        RowLayout {
                            spacing: 8
                            Pill {
                                tint: root.chordsReady ? Theme.success : Theme.goldInk
                                text: root.chordsReady ? qsTr("Drag chords ready") : qsTr("Drag chords not set up")
                            }
                            Pill {
                                tint: root.toolsReady ? Theme.success : Theme.goldInk
                                text: root.toolsReady ? qsTr("Tools ready") : qsTr("Some tools missing")
                            }
                        }
                        Note {
                            text: qsTr("You can open this tour again from the menu.")
                        }
                    }
                }
            }
        }

        // Step dots + navigation
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.bottomMargin: 24
            spacing: 10

            Row {
                spacing: 6
                Repeater {
                    model: root.pageCount
                    delegate: AbstractButton {
                        id: dot
                        required property int index
                        width: root.page === index ? 23 : 6
                        height: 24
                        focusPolicy: Qt.NoFocus
                        Accessible.name: qsTr("Step %1 of %2: %3").arg(index + 1).arg(root.pageCount).arg(root.names[index])
                        onClicked: root.page = index
                        Behavior on width { NumberAnimation { duration: Theme.quick } }
                        background: Rectangle {
                            anchors.verticalCenter: parent.verticalCenter
                            width: dot.width
                            height: 6
                            radius: 3
                            color: root.page === dot.index ? Theme.gold
                                                           : Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, 0.18)
                        }
                    }
                }
            }
            Text {
                text: (root.page + 1) + " / " + root.pageCount
                font.pixelSize: Theme.sizeTiny
                color: Theme.secondaryText
            }
            Item { Layout.fillWidth: true }
            NButton {
                kind: "quiet"
                text: qsTr("Back")
                visible: root.page > 0
                onClicked: root.back()
            }
            NButton {
                kind: "primary"
                text: root.page === root.pageCount - 1 ? qsTr("Open Nashi") : qsTr("Continue")
                onClicked: root.page === root.pageCount - 1 ? root.close() : root.next()
            }
        }
    }
}
