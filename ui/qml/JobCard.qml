pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Progress and result of one job.
Frame {
    id: card

    property JobRunner runner: null

    readonly property bool ok: runner !== null && runner.state === JobRunner.Succeeded
    readonly property bool failed: runner !== null && runner.state === JobRunner.Failed
    readonly property bool cancelled: runner !== null && runner.state === JobRunner.Cancelled

    padding: 10

    ColumnLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                font.bold: true
                text: card.runner ? card.runner.title : ""
            }
            Label {
                visible: card.runner !== null && card.runner.running && card.runner.total > 1
                opacity: 0.7
                text: card.runner ? qsTr("%1 of %2").arg(Math.min(card.runner.currentIndex + 1, card.runner.total)).arg(card.runner.total) : ""
            }
        }

        ProgressBar {
            Layout.fillWidth: true
            visible: card.runner !== null && card.runner.running
            from: 0
            to: 1
            value: card.runner ? card.runner.overallProgress : 0
            indeterminate: card.runner !== null && card.runner.indeterminate
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
            color: card.failed ? "#da4453" : palette.windowText
            opacity: card.failed ? 1.0 : 0.8
            text: {
                if (!card.runner) return ""
                if (card.runner.running) return card.runner.message.length > 0 ? card.runner.message : qsTr("Working…")
                if (card.cancelled) return qsTr("Cancelled")
                if (card.failed) return card.runner.errorSummary.length > 0 ? card.runner.errorSummary : qsTr("Failed")
                if (card.runner.outputs.length === 1) return qsTr("Saved %1").arg(App.fileName(card.runner.outputs[0]))
                return qsTr("Saved %n file(s)", "", card.runner.outputs.length)
            }
        }

        // Text results (e.g. a decoded QR code), selectable and copyable.
        Repeater {
            model: card.runner ? card.runner.outputMessages : []
            delegate: RowLayout {
                id: msgRow
                required property string modelData
                Layout.fillWidth: true
                TextField {
                    Layout.fillWidth: true
                    readOnly: true
                    selectByMouse: true
                    text: msgRow.modelData
                }
                ToolButton {
                    icon.name: "edit-copy"
                    text: qsTr("Copy")
                    display: AbstractButton.IconOnly
                    onClicked: App.actions.copyText(msgRow.modelData)
                    ToolTip.visible: hovered
                    ToolTip.text: text
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            Item { Layout.fillWidth: true }
            Button {
                visible: card.runner !== null && card.runner.running
                text: qsTr("Cancel")
                icon.name: "process-stop"
                onClicked: card.runner.cancel()
            }
            Button {
                visible: card.runner !== null && card.runner.finished && card.runner.outputs.length > 0
                text: qsTr("Open")
                icon.name: "document-open"
                onClicked: {
                    if (card.runner.outputs.length === 1)
                        App.actions.open(card.runner.outputs[0])
                    else
                        App.actions.openFolder(card.runner.outputs)
                }
            }
            Button {
                visible: card.runner !== null && card.runner.finished && card.runner.outputs.length > 0
                text: qsTr("Show in folder")
                icon.name: "folder-open"
                onClicked: App.actions.reveal(card.runner.outputs)
            }
        }
    }
}
