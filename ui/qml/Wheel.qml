pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import NashiApp

// The pie menu. Geometry comes from WheelGeometry so drawing and hit-testing
// agree: slice 0 is centred at 12 o'clock, slices go clockwise.
Item {
    id: pie

    property int stage: OverlayController.Hidden
    property string mode: "convert"
    property Selection selection: null
    property real centerX: width / 2
    property real centerY: height / 2
    property int hoveredIndex: -1
    property bool overCenter: false

    readonly property WheelModel model: selection ? selection.model : null
    readonly property bool showItems: (stage === OverlayController.Live || stage === OverlayController.Pick)
                                      && selection !== null && selection.ready
    readonly property int count: showItems && model ? model.count : 0
    readonly property real innerRadius: WheelGeometry.innerRadius
    readonly property real outerRadius: showItems ? WheelGeometry.outerRadius(count) : 118
    readonly property var hoveredItem: hoveredIndex >= 0 && model ? model.get(hoveredIndex) : null

    signal sliceClicked(int index)
    signal cancelClicked()

    anchors.fill: parent

    function modeTitle(m) {
        if (m === "tools") return qsTr("Tools")
        if (m === "actions") return qsTr("Actions")
        return qsTr("Convert")
    }

    function dropHint(m) {
        if (m === "tools") return qsTr("Drop here for tools")
        if (m === "actions") return qsTr("Drop here for file actions")
        return qsTr("Drop here to convert")
    }

    // Returns the slice index, -1 (centre: cancel) or -2 (outside).
    function sliceAt(x, y) {
        return WheelGeometry.hit(x - centerX, y - centerY, count, innerRadius, outerRadius)
    }

    function trackPointer(x, y) {
        const s = sliceAt(x, y)
        overCenter = s === -1
        hoveredIndex = showItems ? s : -1
    }

    SystemPalette { id: pal; colorGroup: SystemPalette.Active }

    Item {
        id: disc
        x: pie.centerX - pie.outerRadius
        y: pie.centerY - pie.outerRadius
        width: pie.outerRadius * 2
        height: pie.outerRadius * 2
        scale: pie.visible ? 1 : 0.85
        Behavior on scale { NumberAnimation { duration: 110; easing.type: Easing.OutCubic } }

        Rectangle {
            anchors.fill: parent
            radius: width / 2
            color: Qt.rgba(pal.window.r, pal.window.g, pal.window.b, 0.94)
            border.color: Qt.rgba(pal.text.r, pal.text.g, pal.text.b, 0.18)
            border.width: 1
        }

        // Stage 1: one big drop target labelled with the mode.
        Rectangle {
            visible: !pie.showItems
            anchors.centerIn: parent
            width: parent.width - 16
            height: width
            radius: width / 2
            color: Qt.rgba(pal.highlight.r, pal.highlight.g, pal.highlight.b, 0.22)
            border.color: pal.highlight
            border.width: 2
        }

        Repeater {
            model: pie.showItems ? pie.model : null
            delegate: WheelSlice {
                id: sliceDelegate
                required property int index
                required property string title
                required property string iconName
                required property bool available
                required property string missing
                anchors.fill: parent
                sliceIndex: sliceDelegate.index
                sliceCount: pie.count
                innerRadius: pie.innerRadius
                outerRadius: pie.outerRadius
                label: sliceDelegate.title
                icon: sliceDelegate.iconName
                enabledItem: sliceDelegate.available
                missingText: sliceDelegate.missing
                hovered: pie.hoveredIndex === sliceDelegate.index
                accent: pal.highlight
                textColor: pal.text
                highlightedTextColor: pal.highlightedText
                baseColor: pal.base
            }
        }

        // Centre: cancel zone and status.
        Rectangle {
            anchors.centerIn: parent
            width: pie.innerRadius * 2 - 6
            height: width
            radius: width / 2
            color: pie.overCenter ? Qt.rgba(0.85, 0.2, 0.2, 0.85) : pal.button
            border.color: Qt.rgba(pal.text.r, pal.text.g, pal.text.b, 0.25)

            BusyIndicator {
                anchors.centerIn: parent
                width: parent.width * 0.7
                height: width
                running: pie.selection !== null && pie.selection.loading
                visible: running
            }
            Label {
                anchors.centerIn: parent
                width: parent.width - 8
                visible: !(pie.selection !== null && pie.selection.loading)
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                font.pixelSize: 12
                font.bold: true
                color: pie.overCenter ? "white" : pal.buttonText
                text: pie.overCenter || pie.stage === OverlayController.Pick ? qsTr("Cancel")
                                                                                 : pie.modeTitle(pie.mode)
            }
        }
    }

    // Caption under the wheel: file summary, hovered item, errors.
    Label {
        id: caption
        x: pie.centerX - width / 2
        y: Math.min(pie.height - height - 8, disc.y + disc.height + 10)
        width: Math.min(420, pie.width - 16)
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        padding: 8
        color: pal.windowText
        background: Rectangle { radius: 6; color: Qt.rgba(pal.window.r, pal.window.g, pal.window.b, 0.94) }
        text: {
            if (!pie.selection)
                return ""
            if (pie.selection.error.length > 0)
                return qsTr("Could not read the file types: %1").arg(pie.selection.error)
            if (pie.hoveredItem && !pie.hoveredItem.available)
                return qsTr("%1 needs %2 — install it from the Tools page").arg(pie.hoveredItem.title).arg(pie.hoveredItem.missing)
            if (pie.hoveredItem)
                return pie.hoveredItem.title
            if (!pie.showItems)
                return pie.stage === OverlayController.Pick ? qsTr("Reading files…") : pie.dropHint(pie.mode)
            if (pie.count === 0)
                return qsTr("Nothing to offer for %1").arg(pie.selection.summary)
            return pie.stage === OverlayController.Pick ? qsTr("%1 — click an option").arg(pie.selection.summary)
                                                          : pie.selection.summary
        }
        visible: pie.visible && text.length > 0
    }

    // Pick stage: hover + click selection using the same hit test.
    MouseArea {
        x: disc.x
        y: disc.y
        width: disc.width
        height: disc.height
        enabled: pie.visible && pie.stage === OverlayController.Pick
        hoverEnabled: true
        onPositionChanged: (mouse) => pie.trackPointer(mouse.x + x, mouse.y + y)
        onExited: { pie.hoveredIndex = -1; pie.overCenter = false }
        onClicked: (mouse) => {
            const s = pie.sliceAt(mouse.x + x, mouse.y + y)
            if (s === -1 || s === -2)
                pie.cancelClicked()
            else if (pie.showItems)
                pie.sliceClicked(s)
        }
    }
}
