import QtQuick
import QtQuick.Window
import NashiApp

// One transparent full-screen surface per screen, shown only while a chord
// is active (C++ applies layer-shell or the frameless fallback before show).
Window {
    id: root

    readonly property OverlayController ctl: App.overlay
    readonly property bool isHost: ctl.hostWindow === root
    readonly property int stage: ctl.stage
    readonly property bool dragStage: stage === OverlayController.Target || stage === OverlayController.Live

    visible: false
    color: "transparent"
    title: qsTr("Nashi overlay")

    function clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)) }

    // Pick stage: a light veil; clicking anywhere outside the wheel cancels.
    Rectangle {
        anchors.fill: parent
        color: "black"
        opacity: root.isHost && root.stage === OverlayController.Pick ? 0.18 : 0.0
        Behavior on opacity { NumberAnimation { duration: 120 } }
    }
    MouseArea {
        anchors.fill: parent
        enabled: root.stage === OverlayController.Pick
        onClicked: root.ctl.cancel()
    }

    Wheel {
        id: wheel
        visible: root.isHost && root.stage !== OverlayController.Hidden
        stage: root.stage
        mode: root.ctl.mode
        selection: root.ctl.selection
        centerX: root.clamp(root.ctl.anchor.x, outerRadius + 8, Math.max(outerRadius + 8, root.width - outerRadius - 8))
        centerY: root.clamp(root.ctl.anchor.y, outerRadius + 8, Math.max(outerRadius + 8, root.height - outerRadius - 8))
        onSliceClicked: (index) => root.ctl.choose(index)
        onCancelClicked: root.ctl.cancel()
    }

    DropArea {
        id: drop
        anchors.fill: parent
        enabled: root.dragStage

        // Always Copy: Shift-drag from Dolphin proposes Move, and the source
        // must never delete its file because of us.
        onEntered: (drag) => {
            drag.action = Qt.CopyAction
            drag.accept(Qt.CopyAction)
            root.ctl.dragEntered(root, drag.hasUrls ? drag.urls : [], Qt.point(drag.x, drag.y))
            wheel.trackPointer(drag.x, drag.y)
        }
        onPositionChanged: (drag) => {
            drag.action = Qt.CopyAction
            drag.accept(Qt.CopyAction)
            wheel.trackPointer(drag.x, drag.y)
        }
        onExited: {
            wheel.hoveredIndex = -1
            root.ctl.dragLeft(root)
        }
        onDropped: (drop) => {
            drop.action = Qt.CopyAction
            drop.accept(Qt.CopyAction)
            const slice = root.stage === OverlayController.Live ? wheel.sliceAt(drop.x, drop.y) : -3
            root.ctl.dropped(root, drop.hasUrls ? drop.urls : [], Qt.point(drop.x, drop.y), slice)
        }
    }

    Item {
        anchors.fill: parent
        focus: true
        Keys.onEscapePressed: root.ctl.cancel()
    }
}
