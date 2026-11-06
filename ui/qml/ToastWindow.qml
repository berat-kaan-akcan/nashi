pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import NashiApp

// Small non-focus-stealing progress window (top-right). C++ applies the
// layer-shell anchors or the frameless fallback.
Window {
    id: root

    visible: false
    width: 380
    height: Math.max(60, Math.min(content.implicitHeight + 16, 560))
    color: "transparent"
    title: qsTr("Nashi progress")
    onHeightChanged: App.toast.contentResized()

    // QtMultimedia is optional at runtime, so the effect is created on first use.
    property var doneEffect: null
    property bool doneSoundBroken: false
    function playDoneSound() {
        const platform = Qt.platform.pluginName
        if (doneSoundBroken || platform === "offscreen" || platform === "minimal" || platform === "minimalegl")
            return
        if (!doneEffect) {
            try {
                doneEffect = Qt.createQmlObject(
                    'import QtMultimedia\nSoundEffect { source: "qrc:/qt/qml/NashiApp/resources/job-done.wav" }',
                    root, "doneSound")
            } catch (e) {
                doneSoundBroken = true
                return
            }
        }
        doneEffect.play()
    }
    Connections {
        target: App.jobs
        function onJobFinished(job) {
            if (App.settings.doneSound && job.state === JobRunner.Succeeded)
                root.playDoneSound()
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: 18
        color: Theme.surface
        border.color: Theme.border

        HoverHandler { onHoveredChanged: App.toast.hold(hovered) }

        ColumnLayout {
            id: content
            anchors.fill: parent
            anchors.margins: 10
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 4
                spacing: 8
                NashiMark {
                    size: 22
                    happy: App.jobs.activeCount === 0
                }
                Text {
                    Layout.fillWidth: true
                    font.pixelSize: Theme.sizeBody
                    font.weight: Font.Bold
                    color: Theme.text
                    text: App.jobs.activeCount > 0 ? qsTr("Nashi — %n job(s) running", "", App.jobs.activeCount)
                                                   : qsTr("Nashi")
                }
                NButton {
                    kind: "quiet"
                    compact: true
                    iconName: "window-close"
                    Accessible.name: qsTr("Close")
                    onClicked: App.toast.dismiss()
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Close")
                }
            }

            Repeater {
                model: App.jobs
                delegate: JobCard {
                    required property JobRunner job
                    Layout.fillWidth: true
                    runner: job
                }
            }
        }
    }
}
