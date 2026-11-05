pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Visual PDF page organizer: thumbnail grid with drag-and-drop reordering,
// 90° CW rotation, page duplication, deletion, reset, and text-spec sync.
Item {
    id: root
    objectName: "pdfOrganizer"

    property string filePath: ""
    property int pageCount: 0

    // Each item: { page: int (1-based), rotation: int (0|90|180|270) }
    property var items: []
    property var thumbFiles: []
    property int pagesReqId: -1
    property bool loadingThumbs: false

    // Drag-and-drop state
    property int draggingIndex: -1
    property int hoverSlotIndex: -1
    property real dragDeltaX: 0
    property real dragDeltaY: 0

    readonly property int knownMaxPages: pageCount > 0 ? pageCount : thumbFiles.length

    signal pagesEdited(string spec)

    implicitWidth: 640
    implicitHeight: mainCol.implicitHeight

    function fileUrl(path) {
        if (!path || path.length === 0)
            return ""
        return "file://" + String(path).split("/").map(encodeURIComponent).join("/")
    }

    function pagesString() {
        if (!items || items.length === 0)
            return ""
        return items.map(it => String(it.page)).join(",")
    }

    function rotationsList() {
        if (!items || items.length === 0)
            return []
        return items.map(it => ((Number(it.rotation) % 360) + 360) % 360)
    }

    function hasNonZeroRotation() {
        const rots = rotationsList()
        for (let i = 0; i < rots.length; ++i) {
            if (rots[i] !== 0)
                return true
        }
        return false
    }

    function buildOptions() {
        const spec = pagesString()
        if (spec.length === 0)
            return null
        if (hasNonZeroRotation())
            return { pages: spec, rotations: rotationsList() }
        return { pages: spec }
    }

    function populateDefault(count) {
        if (count <= 0)
            return
        const next = []
        for (let i = 1; i <= count; ++i)
            next.push({ page: i, rotation: 0 })
        items = next
        pagesEdited(pagesString())
    }

    function resetOrganizer() {
        pagesReqId = -1
        loadingThumbs = false
        thumbFiles = []
        items = []
        draggingIndex = -1
        hoverSlotIndex = -1
        dragDeltaX = 0
        dragDeltaY = 0
        if (pageCount > 0)
            populateDefault(pageCount)
        requestThumbnails()
    }

    function resetPages() {
        const count = knownMaxPages > 0 ? knownMaxPages : Math.max(1, items.length)
        populateDefault(count)
    }

    function requestThumbnails() {
        if (!filePath || filePath.length === 0) {
            loadingThumbs = false
            return
        }
        loadingThumbs = true
        pagesReqId = App.preview.request(filePath, "pages", { width: 220 })
    }

    onFilePathChanged: {
        thumbFiles = []
        requestThumbnails()
    }

    onPageCountChanged: {
        if (pageCount > 0 && items.length === 0)
            populateDefault(pageCount)
    }

    Connections {
        target: App.preview
        function onReady(requestId, kind, files, width, height) {
            if (requestId !== root.pagesReqId || kind !== "pages")
                return
            root.loadingThumbs = false
            const list = []
            if (files) {
                for (let i = 0; i < files.length; ++i)
                    list.push(String(files[i]))
            }
            root.thumbFiles = list
            if (root.items.length === 0 && list.length > 0)
                root.populateDefault(list.length)
        }
        function onFailed(requestId, message) {
            if (requestId !== root.pagesReqId)
                return
            root.loadingThumbs = false
        }
    }

    function movePage(fromIdx, toIdx) {
        if (fromIdx < 0 || fromIdx >= items.length || toIdx < 0 || toIdx >= items.length || fromIdx === toIdx)
            return
        const next = items.map(it => ({ page: it.page, rotation: it.rotation }))
        const moved = next.splice(fromIdx, 1)[0]
        next.splice(toIdx, 0, moved)
        items = next
        pagesEdited(pagesString())
    }

    function rotatePage(idx) {
        if (idx < 0 || idx >= items.length)
            return
        const next = items.map(it => ({ page: it.page, rotation: it.rotation }))
        next[idx].rotation = (Number(next[idx].rotation) + 90) % 360
        items = next
        pagesEdited(pagesString())
    }

    function duplicatePage(idx) {
        if (idx < 0 || idx >= items.length)
            return
        const next = items.map(it => ({ page: it.page, rotation: it.rotation }))
        const copy = { page: next[idx].page, rotation: next[idx].rotation }
        next.splice(idx + 1, 0, copy)
        items = next
        pagesEdited(pagesString())
    }

    function deletePage(idx) {
        if (items.length <= 1 || idx < 0 || idx >= items.length)
            return
        const next = items.filter((_, i) => i !== idx).map(it => ({ page: it.page, rotation: it.rotation }))
        items = next
        pagesEdited(pagesString())
    }

    // Expands e.g. "3,1-2" -> [3, 1, 2]; returns null if syntax or bounds invalid.
    function expandSpec(spec) {
        const s = String(spec).replace(/\s+/g, "")
        if (!/^\d+(-\d+)?(,\d+(-\d+)?)*$/.test(s))
            return null
        const maxP = knownMaxPages
        const out = []
        const parts = s.split(",")
        for (let i = 0; i < parts.length; ++i) {
            const token = parts[i]
            if (token.indexOf("-") >= 0) {
                const ab = token.split("-")
                const a = parseInt(ab[0], 10)
                const b = parseInt(ab[1], 10)
                if (a < 1 || b < 1 || (maxP > 0 && (a > maxP || b > maxP)))
                    return null
                if (a <= b) {
                    for (let p = a; p <= b && out.length < 500; ++p)
                        out.push(p)
                } else {
                    for (let p = a; p >= b && out.length < 500; --p)
                        out.push(p)
                }
            } else {
                const n = parseInt(token, 10)
                if (n < 1 || (maxP > 0 && n > maxP))
                    return null
                out.push(n)
            }
        }
        return out.length > 0 ? out : null
    }

    function syncFromSpec(spec) {
        const expanded = expandSpec(spec)
        if (!expanded)
            return false
        // If current items already match the expanded page numbers, keep rotations intact.
        if (expanded.length === items.length) {
            let same = true
            for (let i = 0; i < expanded.length; ++i) {
                if (items[i].page !== expanded[i]) {
                    same = false
                    break
                }
            }
            if (same)
                return true
        }
        // Build updated card list, preserving rotations from unused matching cards in order.
        const pool = items.map(it => ({ page: it.page, rotation: it.rotation, used: false }))
        const next = []
        for (let i = 0; i < expanded.length; ++i) {
            const p = expanded[i]
            let rot = 0
            for (let j = 0; j < pool.length; ++j) {
                if (!pool[j].used && pool[j].page === p) {
                    rot = pool[j].rotation
                    pool[j].used = true
                    break
                }
            }
            next.push({ page: p, rotation: rot })
        }
        items = next
        return true
    }

    ColumnLayout {
        id: mainCol
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 12

        // Header toolbar
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Pill {
                text: qsTr("%1 page(s)").arg(root.items.length)
                tint: Theme.forest
            }

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                font.pixelSize: Theme.sizeSmall
                color: Theme.secondaryText
                text: root.loadingThumbs ? qsTr("Rendering page previews…")
                                         : qsTr("Drag cards to reorder · rotate, duplicate or remove pages")
            }

            NButton {
                objectName: "pdfResetButton"
                compact: true
                kind: "secondary"
                iconName: "edit-undo"
                text: qsTr("Reset")
                enabled: root.items.length > 0
                onClicked: root.resetPages()
            }
        }

        // Responsive card grid supporting drag-and-drop reordering
        Item {
            id: gridStage
            objectName: "pdfGrid"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.max(cardH, rowsCount * (cardH + gap) - gap)

            readonly property int cardW: 164
            readonly property int cardH: 224
            readonly property int gap: 12
            readonly property int cols: Math.max(1, Math.floor((Math.max(cardW, width) + gap) / (cardW + gap)))
            readonly property int rowsCount: Math.max(1, Math.ceil(Math.max(1, root.items.length) / cols))

            function slotX(slot) {
                return (slot % cols) * (cardW + gap)
            }

            function slotY(slot) {
                return Math.floor(slot / cols) * (cardH + gap)
            }

            function slotAt(cx, cy) {
                if (root.items.length <= 1)
                    return 0
                const col = Math.max(0, Math.min(cols - 1, Math.floor(cx / (cardW + gap))))
                const row = Math.max(0, Math.floor(cy / (cardH + gap)))
                return Math.max(0, Math.min(root.items.length - 1, row * cols + col))
            }

            function visualSlotFor(idx) {
                const d = root.draggingIndex
                const t = root.hoverSlotIndex
                if (d < 0 || t < 0 || d === t || idx === d)
                    return idx
                if (d < t && idx > d && idx <= t)
                    return idx - 1
                if (d > t && idx >= t && idx < d)
                    return idx + 1
                return idx
            }

            // Drop target highlight frame during drag
            Rectangle {
                visible: root.draggingIndex >= 0 && root.hoverSlotIndex >= 0
                x: gridStage.slotX(Math.max(0, root.hoverSlotIndex))
                y: gridStage.slotY(Math.max(0, root.hoverSlotIndex))
                width: gridStage.cardW
                height: gridStage.cardH
                radius: Theme.radiusRow
                color: Qt.rgba(Theme.gold.r, Theme.gold.g, Theme.gold.b, 0.14)
                border.width: 2
                border.color: Theme.gold
            }

            Repeater {
                model: root.items.length
                delegate: Rectangle {
                    id: pageCard
                    required property int index

                    readonly property var entry: index >= 0 && index < root.items.length
                                                 ? root.items[index]
                                                 : ({ page: 1, rotation: 0 })
                    readonly property int origPage: Number(entry.page)
                    readonly property int rotationDeg: Number(entry.rotation)
                    readonly property bool isDragging: index === root.draggingIndex
                    readonly property int visSlot: gridStage.visualSlotFor(index)
                    readonly property string thumbPath: (origPage >= 1 && origPage <= root.thumbFiles.length)
                                                        ? String(root.thumbFiles[origPage - 1]) : ""

                    x: isDragging ? (gridStage.slotX(index) + root.dragDeltaX) : gridStage.slotX(visSlot)
                    y: isDragging ? (gridStage.slotY(index) + root.dragDeltaY) : gridStage.slotY(visSlot)
                    width: gridStage.cardW
                    height: gridStage.cardH
                    z: isDragging ? 50 : 1
                    scale: isDragging ? 1.03 : 1.0
                    radius: Theme.radiusRow
                    color: isDragging ? Theme.surface : Theme.inset
                    border.width: isDragging ? 2 : 1
                    border.color: isDragging ? Theme.gold : Theme.border

                    Behavior on x {
                        enabled: !pageCard.isDragging && Theme.motion
                        NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic }
                    }
                    Behavior on y {
                        enabled: !pageCard.isDragging && Theme.motion
                        NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic }
                    }

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 10
                        spacing: 6

                        // Top badges: output position + original page # + rotation
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 4

                            Text {
                                text: String(pageCard.index + 1) + "."
                                font.pixelSize: Theme.sizeSmall
                                font.weight: Font.Bold
                                color: Theme.secondaryText
                            }

                            Item { Layout.fillWidth: true }

                            Pill {
                                visible: pageCard.rotationDeg !== 0
                                text: pageCard.rotationDeg + "°"
                                tint: Theme.goldInk
                            }

                            Pill {
                                text: "#" + pageCard.origPage
                                tint: Theme.forest
                            }
                        }

                        // Thumbnail box (draggable to reorder)
                        Item {
                            id: thumbStage
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            readonly property bool quarterTurn: (pageCard.rotationDeg % 180) !== 0
                            readonly property real paperBaseW: 96
                            readonly property real paperBaseH: 132
                            readonly property real rotFit: quarterTurn
                                ? Math.min((width - 8) / paperBaseH, (height - 8) / paperBaseW)
                                : Math.min((width - 8) / paperBaseW, (height - 8) / paperBaseH)

                            Rectangle {
                                id: paperSheet
                                anchors.centerIn: parent
                                width: Math.max(24, thumbStage.paperBaseW * Math.max(0.4, thumbStage.rotFit))
                                height: Math.max(32, thumbStage.paperBaseH * Math.max(0.4, thumbStage.rotFit))
                                rotation: pageCard.rotationDeg
                                color: "#fffdf8"
                                border.width: 1
                                border.color: Qt.rgba(0, 0, 0, 0.16)
                                radius: 4
                                clip: true

                                Behavior on rotation {
                                    enabled: Theme.motion
                                    NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic }
                                }

                                // Fallback vector document sheet while PNG loads
                                Column {
                                    id: sheetStubCol
                                    anchors.fill: parent
                                    anchors.margins: 10
                                    spacing: 6
                                    visible: thumbImg.status !== Image.Ready

                                    Rectangle {
                                        width: sheetStubCol.width * 0.55
                                        height: 5
                                        radius: 2
                                        color: Qt.rgba(0.19, 0.33, 0.23, 0.35)
                                    }
                                    Repeater {
                                        model: 5
                                        delegate: Rectangle {
                                            required property int index
                                            width: sheetStubCol.width * (index === 4 ? 0.65 : 0.92)
                                            height: 3
                                            radius: 1.5
                                            color: Qt.rgba(0.15, 0.18, 0.14, 0.16)
                                        }
                                    }
                                    Text {
                                        anchors.horizontalCenter: sheetStubCol.horizontalCenter
                                        text: "#" + pageCard.origPage
                                        font.pixelSize: Theme.sizeHeading
                                        font.weight: Font.Bold
                                        color: Qt.rgba(0.19, 0.33, 0.23, 0.55)
                                    }
                                }

                                Image {
                                    id: thumbImg
                                    anchors.fill: parent
                                    anchors.margins: 1
                                    source: pageCard.thumbPath.length > 0 ? root.fileUrl(pageCard.thumbPath) : ""
                                    fillMode: Image.PreserveAspectFit
                                    asynchronous: true
                                    smooth: true
                                }
                            }

                            MouseArea {
                                id: cardDragArea
                                anchors.fill: parent
                                // The options page scrolls; a drag here belongs to the card.
                                preventStealing: true
                                cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor

                                property real pressX: 0
                                property real pressY: 0

                                onPressed: mouse => {
                                    const p = mapToItem(gridStage, mouse.x, mouse.y)
                                    pressX = p.x
                                    pressY = p.y
                                    root.draggingIndex = pageCard.index
                                    root.hoverSlotIndex = pageCard.index
                                    root.dragDeltaX = 0
                                    root.dragDeltaY = 0
                                }

                                onPositionChanged: mouse => {
                                    if (!pressed || root.draggingIndex !== pageCard.index)
                                        return
                                    const p = mapToItem(gridStage, mouse.x, mouse.y)
                                    root.dragDeltaX = p.x - pressX
                                    root.dragDeltaY = p.y - pressY
                                    const cx = pageCard.x + pageCard.width / 2
                                    const cy = pageCard.y + pageCard.height / 2
                                    root.hoverSlotIndex = gridStage.slotAt(cx, cy)
                                }

                                onReleased: {
                                    const from = root.draggingIndex
                                    const to = root.hoverSlotIndex
                                    root.draggingIndex = -1
                                    root.hoverSlotIndex = -1
                                    root.dragDeltaX = 0
                                    root.dragDeltaY = 0
                                    if (from >= 0 && to >= 0 && from !== to)
                                        root.movePage(from, to)
                                }

                                onCanceled: {
                                    root.draggingIndex = -1
                                    root.hoverSlotIndex = -1
                                    root.dragDeltaX = 0
                                    root.dragDeltaY = 0
                                }
                            }
                        }

                        // Card action strip: Left, Rotate, Duplicate, Delete, Right
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            NButton {
                                compact: true
                                kind: "quiet"
                                leftPadding: 6; rightPadding: 6
                                topPadding: 4; bottomPadding: 4
                                text: "←"
                                enabled: pageCard.index > 0
                                Accessible.name: qsTr("Move left")
                                ToolTip.visible: hovered
                                ToolTip.text: qsTr("Move earlier")
                                onClicked: root.movePage(pageCard.index, pageCard.index - 1)
                            }

                            NButton {
                                compact: true
                                kind: "quiet"
                                leftPadding: 6; rightPadding: 6
                                topPadding: 4; bottomPadding: 4
                                text: "↻"
                                Accessible.name: qsTr("Rotate 90°")
                                ToolTip.visible: hovered
                                ToolTip.text: qsTr("Rotate 90° clockwise")
                                onClicked: root.rotatePage(pageCard.index)
                            }

                            NButton {
                                compact: true
                                kind: "quiet"
                                leftPadding: 6; rightPadding: 6
                                topPadding: 4; bottomPadding: 4
                                text: "+"
                                Accessible.name: qsTr("Duplicate page")
                                ToolTip.visible: hovered
                                ToolTip.text: qsTr("Duplicate page")
                                onClicked: root.duplicatePage(pageCard.index)
                            }

                            NButton {
                                compact: true
                                kind: "quiet"
                                leftPadding: 6; rightPadding: 6
                                topPadding: 4; bottomPadding: 4
                                text: "×"
                                enabled: root.items.length > 1
                                Accessible.name: qsTr("Delete page")
                                ToolTip.visible: hovered
                                ToolTip.text: qsTr("Remove page")
                                onClicked: root.deletePage(pageCard.index)
                            }

                            NButton {
                                compact: true
                                kind: "quiet"
                                leftPadding: 6; rightPadding: 6
                                topPadding: 4; bottomPadding: 4
                                text: "→"
                                enabled: pageCard.index < root.items.length - 1
                                Accessible.name: qsTr("Move right")
                                ToolTip.visible: hovered
                                ToolTip.text: qsTr("Move later")
                                onClicked: root.movePage(pageCard.index, pageCard.index + 1)
                            }
                        }
                    }
                }
            }
        }
    }
}
