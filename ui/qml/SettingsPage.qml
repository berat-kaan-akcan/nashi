import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import NashiApp

Page {
    id: page
    padding: 12

    readonly property Settings settings: App.settings

    FolderDialog {
        id: folderDialog
        title: qsTr("Output folder")
        onAccepted: page.settings.customOutputDir = page.settings.urlToLocalPath(selectedFolder)
    }

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: parent.width
            spacing: 14

            GroupBox {
                Layout.fillWidth: true
                title: qsTr("Language")
                ComboBox {
                    id: lang
                    width: Math.min(parent.width, 280)
                    textRole: "text"
                    valueRole: "value"
                    model: [
                        { value: "auto", text: qsTr("System default") },
                        { value: "en", text: "English" },
                        { value: "tr", text: "Türkçe" }
                    ]
                    currentIndex: Math.max(0, indexOfValue(page.settings.language))
                    onActivated: page.settings.language = currentValue
                }
            }

            GroupBox {
                Layout.fillWidth: true
                title: qsTr("Output")
                ColumnLayout {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    RadioButton {
                        text: qsTr("Save next to the original file")
                        checked: !page.settings.useCustomOutputDir
                        onToggled: if (checked) page.settings.useCustomOutputDir = false
                    }
                    RadioButton {
                        id: customRadio
                        text: qsTr("Save into a folder:")
                        checked: page.settings.useCustomOutputDir
                        onToggled: if (checked) page.settings.useCustomOutputDir = true
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        enabled: customRadio.checked
                        TextField {
                            Layout.fillWidth: true
                            placeholderText: qsTr("Choose a folder")
                            text: page.settings.customOutputDir
                            onEditingFinished: page.settings.customOutputDir = text
                        }
                        Button {
                            text: qsTr("Browse…")
                            icon.name: "folder-open"
                            onClicked: folderDialog.open()
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        opacity: 0.7
                        text: qsTr("Existing files are never overwritten: a number is added to the name instead.")
                    }
                }
            }

            GroupBox {
                Layout.fillWidth: true
                title: qsTr("Notifications")
                ColumnLayout {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    Switch {
                        text: qsTr("Desktop notifications when a job finishes")
                        checked: page.settings.notifications
                        onToggled: page.settings.notifications = checked
                    }
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        opacity: 0.7
                        text: qsTr("Sent when the progress popup is closed or the job took longer than 10 seconds.")
                    }
                }
            }

            GroupBox {
                Layout.fillWidth: true
                title: qsTr("Drag chords")
                ColumnLayout {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        text: App.inputd.available
                              ? qsTr("nashi-inputd is running (version %1).").arg(App.inputd.version.length > 0 ? App.inputd.version : "?")
                              : qsTr("nashi-inputd is not running. Enable it with: systemctl --user enable --now nashi-inputd")
                    }
                    Switch {
                        text: qsTr("Show the wheel when I hold Shift, Ctrl or Alt while dragging")
                        enabled: App.inputd.available
                        checked: App.inputd.available && App.inputd.enabled
                        onToggled: App.inputd.setEnabled(checked)
                    }
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        opacity: 0.7
                        text: App.layerShell ? qsTr("Overlay: Wayland layer-shell.")
                                             : qsTr("Overlay: full-screen window fallback (no layer-shell).")
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                opacity: 0.6
                text: qsTr("Nashi %1 · nashi-cli: %2").arg(App.version).arg(App.cliPath.length > 0 ? App.cliPath : qsTr("not found"))
                elide: Text.ElideMiddle
            }
        }
    }
}
