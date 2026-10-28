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

    SystemPalette { id: pal; colorGroup: SystemPalette.Active }

    Rectangle {
        anchors.fill: parent
        radius: 10
        color: pal.window
        border.color: Qt.rgba(pal.text.r, pal.text.g, pal.text.b, 0.2)

        HoverHandler { onHoveredChanged: App.toast.hold(hovered) }

        ColumnLayout {
            id: content
            anchors.fill: parent
            anchors.margins: 8
            spacing: 6

            RowLayout {
                Layout.fillWidth: true
                Image {
                    source: "image://icon/io.github.beratkaanakcan.Nashi"
                    sourceSize: Qt.size(18, 18)
                }
                Label {
                    Layout.fillWidth: true
                    font.bold: true
                    color: pal.windowText
                    text: App.jobs.activeCount > 0 ? qsTr("Nashi — %n job(s) running", "", App.jobs.activeCount)
                                                   : qsTr("Nashi")
                }
                ToolButton {
                    icon.name: "window-close"
                    text: qsTr("Close")
                    display: AbstractButton.IconOnly
                    onClicked: App.toast.dismiss()
                    ToolTip.visible: hovered
                    ToolTip.text: text
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
