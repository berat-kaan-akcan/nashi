import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

ApplicationWindow {
    id: root

    visible: false
    width: 960
    height: 720
    minimumWidth: 700
    minimumHeight: 560
    title: qsTr("Nashi")
    color: Theme.canvas

    // Standard controls (text fields, switches…) pick up the Nashi colours.
    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.surface
    palette.alternateBase: Theme.inset
    palette.text: Theme.text
    palette.button: Theme.surface
    palette.buttonText: Theme.text
    palette.highlight: Theme.gold
    palette.highlightedText: Theme.buttonInk
    palette.placeholderText: Theme.tertiaryText
    palette.mid: Theme.border

    header: Item {
        implicitHeight: bar.implicitHeight + 34

        RowLayout {
            id: bar
            anchors.fill: parent
            anchors.leftMargin: 28
            anchors.rightMargin: 28
            anchors.topMargin: 19
            anchors.bottomMargin: 15
            spacing: 8

            NashiMark { size: 40 }
            Text {
                text: "Nashi"
                font.pixelSize: 19
                font.weight: Font.Bold
                font.letterSpacing: -0.5
                color: Theme.text
            }
            Item { Layout.fillWidth: true }

            // Page switcher; `currentIndex` mirrors the old TabBar API (tests use it).
            Row {
                id: tabs
                objectName: "mainTabs"
                property int currentIndex: 0
                spacing: 4
                Repeater {
                    model: [
                        qsTr("Home"),
                        qsTr("Library"),
                        App.deps.missingCount > 0 ? qsTr("Tools (%1 missing)").arg(App.deps.missingCount) : qsTr("Tools")
                    ]
                    delegate: NButton {
                        required property int index
                        required property string modelData
                        compact: true
                        kind: tabs.currentIndex === index ? "secondary" : "quiet"
                        text: modelData
                        ToolTip.visible: hovered
                        ToolTip.text: "Ctrl+" + (index + 1)
                        onClicked: tabs.currentIndex = index
                    }
                }
            }
            Pill {
                text: qsTr("On your computer")
                iconName: "object-locked"
            }
            NButton {
                objectName: "floatToggleButton"
                compact: true
                kind: App.shelf.visible ? "primary" : "quiet"
                iconName: "pin"
                text: App.shelf.count > 0 ? qsTr("Float (%1)").arg(App.shelf.count) : qsTr("Float")
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Toggle Float Shelf")
                onClicked: App.toggleShelf()
            }
            NButton {
                compact: true
                kind: tabs.currentIndex === 3 ? "secondary" : "quiet"
                iconName: "configure"
                text: ""
                Accessible.name: qsTr("Settings")
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Settings") + " · Ctrl+,"
                onClicked: tabs.currentIndex = 3
            }
        }
    }

    StackLayout {
        anchors.fill: parent
        currentIndex: tabs.currentIndex
        DropPage { id: dropPage }
        RecentPage { id: recentPage }
        DepsPage {}
        SettingsPage {}
    }

    // Keyboard shortcuts. Editable fields keep their own Ctrl+V.
    readonly property bool editingText: activeFocusItem instanceof TextInput
                                        || activeFocusItem instanceof TextEdit
    function showLibrarySearch() {
        tabs.currentIndex = 1
        recentPage.focusSearch()
    }
    Connections {
        target: App
        function onHomeTabRequested() {
            tabs.currentIndex = 0
        }
    }
    Shortcut { sequence: "Ctrl+1"; onActivated: tabs.currentIndex = 0 }
    Shortcut { sequence: "Ctrl+2"; onActivated: root.showLibrarySearch() }
    Shortcut { sequence: "Ctrl+3"; onActivated: tabs.currentIndex = 2 }
    Shortcut { sequence: "Ctrl+,"; onActivated: tabs.currentIndex = 3 }
    Shortcut { sequences: [StandardKey.Find]; onActivated: root.showLibrarySearch() }
    Shortcut {
        sequences: [StandardKey.Open]
        onActivated: {
            tabs.currentIndex = 0
            dropPage.chooseFiles()
        }
    }
    Shortcut {
        sequences: [StandardKey.Paste]
        enabled: !root.editingText
        onActivated: {
            tabs.currentIndex = 0
            dropPage.paste()
        }
    }

    footer: Rectangle {
        implicitHeight: 34
        color: Theme.canvas
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 28
            anchors.rightMargin: 28
            anchors.bottomMargin: 10
            spacing: 6

            Image {
                visible: status.message.length > 0
                source: "image://icon/dialog-information"
                sourceSize: Qt.size(24, 24)
                Layout.preferredWidth: 12
                Layout.preferredHeight: 12
                opacity: 0.7
            }
            Text {
                id: status
                Layout.fillWidth: true
                elide: Text.ElideRight
                font.pixelSize: Theme.sizeTiny
                font.weight: Font.Medium
                color: message.length > 0 ? Theme.text : Theme.secondaryText
                property string message: ""
                text: message.length > 0 ? message
                      : App.inputd.available
                        ? (App.inputd.enabled ? qsTr("A little less file fuss. A little more flow.")
                                              : qsTr("Drag chords are disabled."))
                        : qsTr("nashi-inputd is not running — drag chords are unavailable; drop files here instead.")

                Timer {
                    id: messageTimer
                    interval: 4000
                    onTriggered: status.message = ""
                }
                Connections {
                    target: App
                    function onMessage(text) {
                        status.message = text
                        messageTimer.restart()
                    }
                }
            }
            Text {
                text: qsTr("Made for Linux")
                font.pixelSize: Theme.sizeTiny
                font.weight: Font.Medium
                color: Theme.tertiaryText
            }
        }
    }
}
