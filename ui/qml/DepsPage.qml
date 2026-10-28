pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// External tools reported by `nashi-cli deps --json`.
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
                text: qsTr("External tools")
            }
            BusyIndicator {
                running: App.deps.loading
                visible: running
                Layout.preferredHeight: 24
                Layout.preferredWidth: 24
            }
            Button {
                text: qsTr("Check again")
                icon.name: "view-refresh"
                enabled: !App.deps.loading
                onClicked: App.deps.refresh()
            }
        }

        Label {
            Layout.fillWidth: true
            visible: App.cliPath.length === 0
            wrapMode: Text.Wrap
            color: "#da4453"
            text: qsTr("nashi-cli was not found. Install Nashi properly or set NASHI_CLI.")
        }
        Label {
            Layout.fillWidth: true
            visible: App.deps.error.length > 0
            wrapMode: Text.Wrap
            color: "#da4453"
            text: App.deps.error
        }

        Frame {
            Layout.fillWidth: true
            visible: App.deps.missingCount > 0 && App.deps.installCommand.length > 0
            ColumnLayout {
                anchors.left: parent.left
                anchors.right: parent.right
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: qsTr("Some features need tools that are not installed. Install them with:")
                }
                RowLayout {
                    Layout.fillWidth: true
                    TextField {
                        id: cmd
                        Layout.fillWidth: true
                        readOnly: true
                        selectByMouse: true
                        font.family: "monospace"
                        text: App.deps.installCommand
                    }
                    Button {
                        text: qsTr("Copy")
                        icon.name: "edit-copy"
                        onClicked: App.actions.copyText(cmd.text)
                    }
                }
            }
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: App.deps
            spacing: 2
            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: row
                required property string toolId
                required property bool available
                required property string version
                required property string path
                required property string packageName
                required property bool optional
                required property string usedFor
                width: ListView.view.width

                contentItem: RowLayout {
                    spacing: 10
                    Rectangle {
                        Layout.preferredWidth: 10
                        Layout.preferredHeight: 10
                        radius: 5
                        color: row.available ? "#27ae60" : (row.optional ? "#f67400" : "#da4453")
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        Label {
                            Layout.fillWidth: true
                            font.bold: true
                            text: row.toolId + (row.optional ? qsTr(" (optional)") : "")
                        }
                        Label {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            opacity: 0.7
                            font.pixelSize: 11
                            text: row.available
                                  ? qsTr("%1 · %2").arg(row.version.length > 0 ? row.version : qsTr("installed")).arg(row.path)
                                  : qsTr("Missing — package: %1").arg(row.packageName)
                        }
                        Label {
                            Layout.fillWidth: true
                            visible: row.usedFor.length > 0
                            elide: Text.ElideRight
                            opacity: 0.6
                            font.pixelSize: 11
                            text: qsTr("Used for: %1").arg(row.usedFor)
                        }
                    }
                }
            }
        }
    }
}
