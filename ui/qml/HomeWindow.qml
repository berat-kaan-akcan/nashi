import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

ApplicationWindow {
    id: root

    visible: false
    width: 900
    height: 620
    minimumWidth: 640
    minimumHeight: 460
    title: qsTr("Nashi")

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            TabBar {
                id: tabs
                objectName: "mainTabs"
                Layout.fillWidth: true
                TabButton { text: qsTr("Convert") }
                TabButton { text: qsTr("Recent") }
                TabButton {
                    text: App.deps.missingCount > 0 ? qsTr("Tools (%1 missing)").arg(App.deps.missingCount) : qsTr("Tools")
                }
                TabButton { text: qsTr("Settings") }
            }
        }
    }

    StackLayout {
        anchors.fill: parent
        currentIndex: tabs.currentIndex
        DropPage {}
        RecentPage {}
        DepsPage {}
        SettingsPage {}
    }

    footer: Label {
        id: status
        padding: 6
        elide: Text.ElideRight
        opacity: 0.8
        text: App.inputd.available
              ? (App.inputd.enabled ? qsTr("Drag a file and hold Shift (convert), Ctrl (tools) or Alt (actions).")
                                    : qsTr("Drag chords are disabled."))
              : qsTr("nashi-inputd is not running — drag chords are unavailable; drop files here instead.")

        Timer {
            id: messageTimer
            interval: 4000
            onTriggered: status.message = ""
        }
        property string message: ""
        Connections {
            target: App
            function onMessage(text) {
                status.message = text
                messageTimer.restart()
            }
        }
        states: State {
            when: status.message.length > 0
            PropertyChanges { status.text: status.message; status.opacity: 1.0 }
        }
    }
}
