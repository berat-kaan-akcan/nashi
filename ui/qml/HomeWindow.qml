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

            // Header after upstream Windows: setup state, Home, Library,
            // "On your computer", Float and Settings. `tabs.currentIndex`
            // keeps the old TabBar API (tests use it): 0 Home, 1 Library,
            // 2 Tools, 3 Settings.
            NButton {
                objectName: "toolsSetupButton"
                compact: true
                selected: tabs.currentIndex === 2
                text: App.deps.missingCount > 0 ? qsTr("Finish setup") : qsTr("Tools ready")
                ToolTip.visible: hovered
                ToolTip.text: (App.deps.missingCount > 0 ? qsTr("%1 tools missing").arg(App.deps.missingCount)
                                                         : qsTr("Tools")) + " · Ctrl+3"
                onClicked: tabs.currentIndex = 2
            }
            Row {
                id: tabs
                objectName: "mainTabs"
                property int currentIndex: 0
                spacing: 6
                Repeater {
                    model: [qsTr("Home"), qsTr("Library")]
                    delegate: NButton {
                        required property int index
                        required property string modelData
                        compact: true
                        selected: tabs.currentIndex === index
                        text: modelData
                        ToolTip.visible: hovered
                        ToolTip.text: "Ctrl+" + (index + 1)
                        onClicked: tabs.currentIndex = index
                    }
                }
            }
            Text {
                Layout.leftMargin: 2
                Layout.rightMargin: 2
                text: qsTr("On your computer")
                font.pixelSize: Theme.sizeSmall
                color: Theme.secondaryText
            }
            NButton {
                objectName: "floatToggleButton"
                compact: true
                kind: App.shelf.visible ? "primary" : "secondary"
                text: App.shelf.count > 0 ? qsTr("Float (%1)").arg(App.shelf.count) : qsTr("Float")
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Toggle Float Shelf")
                onClicked: App.toggleShelf()
            }
            NButton {
                objectName: "settingsButton"
                compact: true
                selected: tabs.currentIndex === 3
                text: qsTr("Settings")
                ToolTip.visible: hovered
                ToolTip.text: "Ctrl+,"
                onClicked: tabs.currentIndex = 3
            }
        }
    }

    StackLayout {
        anchors.fill: parent
        currentIndex: tabs.currentIndex
        DropPage {
            id: dropPage
            onLibraryRequested: tabs.currentIndex = 1
            onToolsRequested: tabs.currentIndex = 2
            onShortcutsRequested: shortcutsDialog.open()
        }
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
    // Same as the KDE global shortcut, so the Home hint works even without it.
    Shortcut { sequence: "Ctrl+Shift+D"; onActivated: App.showDownloadFromClipboard() }

    ShortcutsDialog { id: shortcutsDialog }
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
                        ? (App.inputd.enabled ? qsTr("Files stay on your computer.")
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
        }
    }
}
