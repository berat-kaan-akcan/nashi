pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

Page {
    id: page
    padding: 12

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                font.bold: true
                text: qsTr("Recent outputs")
            }
            Button {
                text: qsTr("Clear list")
                icon.name: "edit-clear-history"
                enabled: App.recent.count > 0
                onClicked: App.recent.clear()
            }
        }

        Label {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: App.recent.count === 0
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            opacity: 0.7
            text: qsTr("Files you create with Nashi appear here.")
        }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: App.recent.count > 0
            clip: true
            model: App.recent
            spacing: 2
            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: row
                required property int index
                required property string path
                required property string name
                required property string folder
                required property string when
                required property bool exists
                width: ListView.view.width
                enabled: true
                onDoubleClicked: if (row.exists) App.actions.open(row.path)

                contentItem: RowLayout {
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        Label {
                            Layout.fillWidth: true
                            elide: Text.ElideMiddle
                            text: row.name
                            font.strikeout: !row.exists
                        }
                        Label {
                            Layout.fillWidth: true
                            elide: Text.ElideMiddle
                            opacity: 0.65
                            font.pixelSize: 11
                            text: row.exists ? row.when + " · " + row.folder : qsTr("No longer exists")
                        }
                    }
                    ToolButton {
                        icon.name: "document-open"
                        text: qsTr("Open")
                        display: AbstractButton.IconOnly
                        enabled: row.exists
                        onClicked: App.actions.open(row.path)
                        ToolTip.visible: hovered
                        ToolTip.text: text
                    }
                    ToolButton {
                        icon.name: "folder-open"
                        text: qsTr("Show in folder")
                        display: AbstractButton.IconOnly
                        enabled: row.exists
                        onClicked: App.actions.reveal([row.path])
                        ToolTip.visible: hovered
                        ToolTip.text: text
                    }
                    ToolButton {
                        icon.name: "list-remove"
                        text: qsTr("Remove from list")
                        display: AbstractButton.IconOnly
                        onClicked: App.recent.remove(row.index)
                        ToolTip.visible: hovered
                        ToolTip.text: text
                    }
                }
            }
        }
    }
}
