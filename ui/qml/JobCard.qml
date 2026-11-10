pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Progress and result of one job.
Rectangle {
    id: card

    property JobRunner runner: null

    readonly property bool ok: runner !== null && runner.state === JobRunner.Succeeded
    readonly property bool failed: runner !== null && runner.state === JobRunner.Failed
    readonly property bool cancelled: runner !== null && runner.state === JobRunner.Cancelled
    readonly property bool running: runner !== null && runner.running

    implicitHeight: column.implicitHeight + 24
    radius: Theme.radiusRow
    color: Theme.canvas
    border.width: 1
    border.color: Theme.border

    ColumnLayout {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 12
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            // Status badge: check when done, cross on failure.
            Rectangle {
                Layout.preferredWidth: 28
                Layout.preferredHeight: 28
                radius: 14
                visible: !card.running
                color: card.failed ? Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, 0.15)
                                   : card.cancelled ? Theme.inset : Theme.sage
                scale: visible || !Theme.motion ? 1 : 0.75
                Behavior on scale { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack } }
                Text {
                    anchors.centerIn: parent
                    text: card.failed ? "✕" : card.cancelled ? "–" : "✓"
                    font.pixelSize: 14
                    font.weight: Font.Bold
                    color: card.failed ? Theme.danger : Theme.forest
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1
                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideMiddle
                    font.pixelSize: Theme.sizeBody
                    font.weight: Font.DemiBold
                    color: Theme.text
                    text: card.runner ? card.runner.title : ""
                }
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    maximumLineCount: 3
                    elide: Text.ElideRight
                    font.pixelSize: Theme.sizeSmall
                    color: card.failed ? Theme.danger : Theme.secondaryText
                    text: {
                        if (!card.runner) return ""
                        if (card.runner.running) return card.runner.message.length > 0 ? card.runner.message : qsTr("Working…")
                        if (card.cancelled) return qsTr("Cancelled")
                        if (card.failed) return card.runner.errorSummary.length > 0 ? card.runner.errorSummary : qsTr("Failed")
                        if (card.runner.outputs.length === 1) return qsTr("Saved %1").arg(App.fileName(card.runner.outputs[0]))
                        return qsTr("Saved %n file(s)", "", card.runner.outputs.length)
                    }
                }
            }
            Text {
                visible: card.running && card.runner.total > 1
                font.pixelSize: Theme.sizeSmall
                color: Theme.secondaryText
                text: card.runner ? qsTr("%1 of %2").arg(Math.min(card.runner.currentIndex + 1, card.runner.total)).arg(card.runner.total) : ""
            }
        }

        // Gold progress bar (indeterminate: a sliding segment).
        Rectangle {
            id: track
            Layout.fillWidth: true
            Layout.preferredHeight: 6
            visible: card.running
            radius: 3
            color: Theme.inset
            clip: true
            readonly property bool indeterminate: card.runner !== null && card.runner.indeterminate
            Rectangle {
                id: fill
                height: parent.height
                radius: 3
                width: track.indeterminate ? parent.width * 0.3
                                           : parent.width * Math.max(0, Math.min(1, card.runner ? card.runner.overallProgress : 0))
                gradient: Gradient {
                    orientation: Gradient.Horizontal
                    GradientStop { position: 0; color: Theme.gold }
                    GradientStop { position: 1; color: Theme.goldEnd }
                }
                Behavior on width { enabled: !track.indeterminate; NumberAnimation { duration: Theme.quick } }
            }
            NumberAnimation {
                target: fill
                property: "x"
                from: -track.width * 0.3
                to: track.width
                duration: 1200
                loops: Animation.Infinite
                running: track.visible && track.indeterminate
                onStopped: fill.x = 0
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
                NButton {
                    compact: true
                    iconName: "edit-copy"
                    Accessible.name: qsTr("Copy")
                    onClicked: App.actions.copyText(msgRow.modelData)
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Copy")
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            visible: card.running || (card.runner !== null && card.runner.finished && card.runner.outputs.length > 0)
                     || (card.runner !== null && card.runner.needsPassword)
            Item { Layout.fillWidth: true }
            NButton {
                objectName: "jobPasswordButton"
                visible: card.runner !== null && card.runner.needsPassword
                compact: true
                kind: "primary"
                iconName: "dialog-password"
                text: qsTr("Enter password")
                onClicked: App.askPassword(card.runner)
            }
            NButton {
                visible: card.running
                compact: true
                iconName: "process-stop"
                text: qsTr("Cancel")
                onClicked: card.runner.cancel()
            }
            NButton {
                visible: card.runner !== null && card.runner.finished && card.runner.outputs.length > 0
                compact: true
                kind: "primary"
                iconName: "document-open"
                text: qsTr("Open")
                onClicked: {
                    if (card.runner.outputs.length === 1)
                        App.actions.open(card.runner.outputs[0])
                    else
                        App.actions.openFolder(card.runner.outputs)
                }
            }
            NButton {
                visible: card.runner !== null && card.runner.finished && card.runner.outputs.length > 0
                compact: true
                iconName: "folder-open"
                text: qsTr("Show in folder")
                onClicked: App.actions.reveal(card.runner.outputs)
            }
            NButton {
                objectName: "jobPinFloatButton"
                visible: card.runner !== null && card.runner.finished && card.runner.outputs.length > 0
                compact: true
                iconName: "pin"
                Accessible.name: qsTr("Pin to Float")
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Pin to Float")
                onClicked: App.addToShelf(card.runner.outputs)
            }
            NButton {
                objectName: "jobShareButton"
                visible: card.runner !== null && card.runner.finished && card.runner.outputs.length > 0 && App.shareAvailable
                compact: true
                iconName: "document-send"
                Accessible.name: qsTr("Share")
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Share")
                onClicked: App.showShare(card.runner.outputs)
            }
        }
    }
}
