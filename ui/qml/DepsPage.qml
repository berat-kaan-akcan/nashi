pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// External tools reported by `nashi-cli deps --json`.
Page {
    id: page
    background: null
    padding: 0

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 28
        anchors.rightMargin: 28
        anchors.bottomMargin: 8
        spacing: 14

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 8
            PageHeading {
                Layout.fillWidth: true
                title: qsTr("Tools")
                subtitle: qsTr("Nashi uses the programs already on your system. Nothing is downloaded.")
            }
            BusyIndicator {
                Layout.alignment: Qt.AlignBottom
                running: App.deps.loading
                visible: running
                Layout.preferredHeight: 24
                Layout.preferredWidth: 24
            }
            NButton {
                Layout.alignment: Qt.AlignBottom
                compact: true
                iconName: "view-refresh"
                text: qsTr("Check again")
                enabled: !App.deps.loading
                onClicked: App.deps.refresh()
            }
        }

        Text {
            Layout.fillWidth: true
            visible: App.cliPath.length === 0
            wrapMode: Text.Wrap
            color: Theme.danger
            text: qsTr("nashi-cli was not found. Install Nashi properly or set NASHI_CLI.")
        }
        Text {
            Layout.fillWidth: true
            visible: App.deps.error.length > 0
            wrapMode: Text.Wrap
            color: Theme.danger
            text: App.deps.error
        }

        SectionBox {
            visible: App.deps.missingCount > 0 && App.deps.installCommand.length > 0
            title: qsTr("Install what's missing")
            Text {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                font.pixelSize: Theme.sizeSmall + 1
                color: Theme.secondaryText
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
                NButton {
                    kind: "primary"
                    compact: true
                    iconName: "edit-copy"
                    text: qsTr("Copy")
                    onClicked: App.actions.copyText(cmd.text)
                }
            }
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: App.deps
            spacing: 8
            ScrollBar.vertical: ScrollBar {}

            delegate: Rectangle {
                id: row
                required property string toolId
                required property bool available
                required property string version
                required property string path
                required property string packageName
                required property bool optional
                required property string usedFor
                width: ListView.view.width
                implicitHeight: info.implicitHeight + 24
                radius: Theme.radiusRow
                color: Theme.surface
                border.width: 1
                border.color: Theme.border

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 12
                    Rectangle {
                        Layout.preferredWidth: 10
                        Layout.preferredHeight: 10
                        radius: 5
                        color: row.available ? Theme.success : (row.optional ? Theme.gold : Theme.danger)
                    }
                    ColumnLayout {
                        id: info
                        Layout.fillWidth: true
                        spacing: 2
                        Text {
                            Layout.fillWidth: true
                            text: row.toolId + (row.optional ? qsTr(" (optional)") : "")
                            font.pixelSize: Theme.sizeBody
                            font.weight: Font.DemiBold
                            color: Theme.text
                        }
                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            font.pixelSize: Theme.sizeSmall
                            color: Theme.secondaryText
                            text: row.available
                                  ? qsTr("%1 · %2").arg(row.version.length > 0 ? row.version : qsTr("installed")).arg(row.path)
                                  : qsTr("Missing — package: %1").arg(row.packageName)
                        }
                        Text {
                            Layout.fillWidth: true
                            visible: row.usedFor.length > 0
                            elide: Text.ElideRight
                            font.pixelSize: Theme.sizeSmall
                            color: Theme.tertiaryText
                            text: qsTr("Used for: %1").arg(row.usedFor)
                        }
                    }
                    Pill {
                        text: row.available ? qsTr("Ready") : row.optional ? qsTr("Optional") : qsTr("Missing")
                        tint: row.available ? Theme.success : row.optional ? Theme.gold : Theme.danger
                        textColor: row.available ? Theme.forest : row.optional ? Theme.goldInk : Theme.danger
                    }
                }
            }
        }
    }
}
