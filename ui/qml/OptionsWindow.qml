pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Visual editors and precision option forms for tools with `needs_editor: true`
// (docs/CLI-PROTOCOL.md). App sets toolId/toolTitle/files, calls reset(), and
// fills `info` from `nashi-cli inspect`.
ApplicationWindow {
    id: win

    property string toolId: ""
    property string toolTitle: ""
    property var files: []
    property var info: ({})

    readonly property string firstFile: files && files.length > 0 ? String(files[0]) : ""
    readonly property real duration: info && info.duration ? Number(info.duration) : 0
    readonly property int mediaWidth: info && info.width ? Number(info.width) : 0
    readonly property int mediaHeight: info && info.height ? Number(info.height) : 0
    readonly property int pageCount: info && info.pages ? Number(info.pages) : 0

    readonly property string group: {
        switch (toolId) {
        case "changeVideoSpeed": return "speed"
        case "trimVideo": case "trimAudio": return "trim"
        case "cropVideo": case "cropImage": return "crop"
        case "editImage": return "edit"
        case "frameImage": return "frame"
        case "redactImage": return "redactImage"
        case "redactVideo": return "redactVideo"
        case "redactAudio": return "redactAudio"
        case "organizePDF": return "pages"
        case "annotateImage": return "annotate"
        }
        return ""
    }
    readonly property bool usesRect: group === "crop" || group === "redactImage" || group === "redactVideo"
    readonly property bool usesRange: group === "trim" || group === "redactVideo"
    readonly property bool usesTimeline: usesRange || group === "redactAudio"
    readonly property bool isVideoTool: toolId === "cropVideo" || toolId === "trimVideo"
                                        || toolId === "redactVideo" || toolId === "changeVideoSpeed"
    readonly property bool isVisualEditor: usesRect || usesTimeline || group === "pages" || group === "annotate"
    readonly property bool textInputFocused: activeFocusItem instanceof TextInput
                                             || activeFocusItem instanceof TextEdit

    // Built options object or an error message string (shown under the form).
    readonly property var result: build()

    visible: false
    width: isVisualEditor ? 880 : 540
    height: isVisualEditor ? 720 : Math.min(720, Math.max(380, body.implicitHeight + 140))
    minimumWidth: 460
    minimumHeight: 320
    title: toolTitle
    color: Theme.canvas
    flags: Qt.Dialog

    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.surface
    palette.text: Theme.text
    palette.button: Theme.surface
    palette.buttonText: Theme.text
    palette.highlight: Theme.gold
    palette.highlightedText: Theme.buttonInk
    palette.placeholderText: Theme.tertiaryText
    palette.mid: Theme.border

    // ---- state ---------------------------------------------------------
    property real speed: 2
    property real rotate: 0
    property string flip: ""
    property string borderColor: "#ffffff"
    property var ranges: [[0, 1]]
    property string audioMode: "mute"

    property bool syncingRect: false
    property bool syncingTime: false
    property bool syncingPages: false

    function reset() {
        speed = 2
        rotate = 0
        flip = ""
        borderColor = "#ffffff"
        ranges = [[0, 1]]
        audioMode = "mute"
        trimSilenceSwitch.checked = false

        syncingTime = true
        startField.text = "0"
        endField.text = ""
        syncingTime = false

        syncingRect = true
        rx.text = "0"; ry.text = "0"; rw.text = ""; rh.text = ""
        syncingRect = false

        resizeW.text = ""; resizeH.text = ""
        borderPx.value = 40

        syncingPages = true
        pagesField.text = ""
        syncingPages = false

        rectEditor.resetEditor()
        timelineEditor.resetEditor()
        pdfOrganizer.resetOrganizer()
        annotateEditor.resetEditor()

        win.width = win.isVisualEditor ? 880 : 540
        win.height = win.isVisualEditor ? 720 : Math.min(720, Math.max(380, body.implicitHeight + 140))
    }

    // Facts from `nashi-cli inspect` (duration, width/height, pages): fills
    // empty fields and visual editors with sensible defaults.
    function applyInfo(map) {
        info = map || ({})

        if (duration > 0 && endField.text.length === 0) {
            syncingTime = true
            endField.text = formatTime(duration)
            timelineEditor.startTime = 0
            timelineEditor.endTime = duration
            syncingTime = false
        }
        if (duration > 0 && group === "redactAudio"
                && ranges.length === 1 && ranges[0][0] === 0 && ranges[0][1] === 1) {
            ranges = [[0, Math.min(1, duration)]]
        }
        if (mediaWidth > 0 && mediaHeight > 0 && rw.text.length === 0) {
            syncingRect = true
            if (group === "crop") {
                rx.text = "0"
                ry.text = "0"
                rw.text = String(mediaWidth)
                rh.text = String(mediaHeight)
                rectEditor.rectX = 0
                rectEditor.rectY = 0
                rectEditor.rectW = mediaWidth
                rectEditor.rectH = mediaHeight
            } else if (group === "redactImage" || group === "redactVideo") {
                const ix = Math.round(mediaWidth * 3 / 8)
                const iy = Math.round(mediaHeight * 3 / 8)
                const iw = Math.max(1, Math.round(mediaWidth / 4))
                const ih = Math.max(1, Math.round(mediaHeight / 4))
                rx.text = String(ix)
                ry.text = String(iy)
                rw.text = String(iw)
                rh.text = String(ih)
                rectEditor.rects = [{ x: ix, y: iy, w: iw, h: ih, style: rectEditor.currentStyle }]
                rectEditor.selectedIndex = 0
            }
            syncingRect = false
        }
        if (pageCount > 0 && pagesField.text.length === 0) {
            if (pdfOrganizer.items.length === 0)
                pdfOrganizer.populateDefault(pageCount)
            syncingPages = true
            pagesField.text = pdfOrganizer.pagesString().length > 0
                              ? pdfOrganizer.pagesString()
                              : (pageCount > 1 ? "1-" + pageCount : "1")
            syncingPages = false
        }
    }

    function syncRectFromFields() {
        if (syncingRect || !usesRect)
            return
        const x = intOf(rx)
        const y = intOf(ry)
        const w = intOf(rw)
        const h = intOf(rh)
        if ([x, y, w, h].some(isNaN) || w <= 0 || h <= 0)
            return
        syncingRect = true
        if (group === "crop") {
            rectEditor.aspectLabel = "free"
            rectEditor.aspectRatio = 0
            rectEditor.rectX = x
            rectEditor.rectY = y
            rectEditor.rectW = w
            rectEditor.rectH = h
        } else {
            const next = rectEditor.rects.slice()
            if (rectEditor.selectedIndex >= 0 && rectEditor.selectedIndex < next.length) {
                const cur = next[rectEditor.selectedIndex]
                next[rectEditor.selectedIndex] = {
                    x: x, y: y, w: w, h: h,
                    style: cur.style || rectEditor.currentStyle
                }
                rectEditor.rects = next
            } else if (next.length === 0) {
                rectEditor.rects = [{ x: x, y: y, w: w, h: h, style: rectEditor.currentStyle }]
                rectEditor.selectedIndex = 0
            }
        }
        syncingRect = false
    }

    function syncTimelineFromFields() {
        if (syncingTime || !usesRange)
            return
        const a = parseTime(startField.text)
        const b = endField.text.trim().length === 0 && duration > 0 ? duration : parseTime(endField.text)
        syncingTime = true
        if (!isNaN(a) && a >= 0)
            timelineEditor.startTime = a
        if (!isNaN(b) && b > 0)
            timelineEditor.endTime = b
        syncingTime = false
    }

    // "1:02:03.5", "62.5" or "1:02" → seconds; NaN when invalid.
    function parseTime(t) {
        const s = String(t).trim().replace(",", ".")
        if (!/^\d+(:\d{1,2}){0,2}(\.\d+)?$/.test(s))
            return NaN
        return s.split(":").reduce((acc, part) => acc * 60 + parseFloat(part), 0)
    }
    function formatTime(sec) {
        const h = Math.floor(sec / 3600)
        const m = Math.floor(sec % 3600 / 60)
        const s = sec % 60
        const ss = (s < 10 ? "0" : "") + s.toFixed(s % 1 === 0 ? 0 : 2)
        return (h > 0 ? h + ":" + (m < 10 ? "0" : "") : "") + m + ":" + ss
    }
    function intOf(field) {
        const v = parseInt(field.text, 10)
        return /^\s*\d+\s*$/.test(field.text) ? v : NaN
    }
    function validateRectObj(r) {
        if ([r.x, r.y, r.w, r.h].some(isNaN))
            return qsTr("Enter whole numbers for the area.")
        if (r.w <= 0 || r.h <= 0)
            return qsTr("Width and height must be greater than zero.")
        if (mediaWidth > 0 && (r.x + r.w > mediaWidth || r.y + r.h > mediaHeight))
            return qsTr("The area must fit inside %1 × %2.").arg(mediaWidth).arg(mediaHeight)
        return r
    }
    function rectOrError() {
        return validateRectObj({ x: intOf(rx), y: intOf(ry), w: intOf(rw), h: intOf(rh) })
    }
    function redactRectsOrError() {
        if (!rectEditor.rects || rectEditor.rects.length === 0)
            return qsTr("Draw or add at least one area to cover.")
        // Also validate active precision fields if a box is selected
        if (rectEditor.selectedIndex >= 0) {
            const fieldCheck = rectOrError()
            if (typeof fieldCheck === "string")
                return fieldCheck
        }
        const out = []
        for (let i = 0; i < rectEditor.rects.length; ++i) {
            const raw = rectEditor.rects[i]
            const checked = validateRectObj({
                x: Number(raw.x),
                y: Number(raw.y),
                w: Number(raw.w),
                h: Number(raw.h)
            })
            if (typeof checked === "string")
                return checked
            out.push({
                x: checked.x,
                y: checked.y,
                w: checked.w,
                h: checked.h,
                style: raw.style ? String(raw.style) : "black"
            })
        }
        return out
    }
    function rangeOrError() {
        const a = parseTime(startField.text)
        const b = endField.text.trim().length === 0 && duration > 0 ? duration : parseTime(endField.text)
        if (isNaN(a) || isNaN(b))
            return qsTr("Use times like 75, 1:15 or 1:02:03.5.")
        if (b <= a)
            return qsTr("The end must be after the start.")
        if (duration > 0 && a >= duration)
            return qsTr("The start is past the end of the file (%1).").arg(formatTime(duration))
        return [a, Math.min(b, duration > 0 ? duration : b)]
    }

    function build() {
        switch (group) {
        case "speed":
            return { speed: speed }
        case "trim": {
            const r = rangeOrError()
            if (typeof r === "string")
                return r
            const trim = { start: r[0], end: r[1] }
            if (toolId === "trimAudio" && trimSilenceSwitch.checked)
                trim.trim_silence = true
            return trim
        }
        case "crop": {
            const r = rectOrError()
            return typeof r === "string" ? r : { rect: r }
        }
        case "redactImage": {
            const list = redactRectsOrError()
            return typeof list === "string" ? list : { rects: list }
        }
        case "redactVideo": {
            const list = redactRectsOrError()
            if (typeof list === "string")
                return list
            const t = rangeOrError()
            if (typeof t === "string")
                return t
            const timed = list.map(r => ({
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                style: r.style,
                t0: t[0],
                t1: t[1]
            }))
            return { rects: timed }
        }
        case "redactAudio": {
            if (!ranges || ranges.length === 0)
                return qsTr("Add at least one range to mute.")
            for (const [a, b] of ranges) {
                if (!(b > a) || a < 0)
                    return qsTr("Each range needs an end after its start.")
            }
            return audioMode === "bleep" ? { ranges: ranges, audio_mode: "bleep" } : { ranges: ranges }
        }
        case "edit": {
            const o = {}
            if (rotate !== 0) o.rotate = rotate
            if (flip.length > 0) o.flip = flip
            const w = resizeW.text.trim().length > 0 ? intOf(resizeW) : 0
            const h = resizeH.text.trim().length > 0 ? intOf(resizeH) : 0
            if (isNaN(w) || isNaN(h))
                return qsTr("Enter whole numbers for the new size.")
            if (w > 0 || h > 0)
                o.resize = { w: w > 0 ? w : null, h: h > 0 ? h : null }
            if (Object.keys(o).length === 0)
                return qsTr("Choose a rotation, a flip or a new size.")
            return o
        }
        case "frame":
            if (!/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(borderColor))
                return qsTr("Use a colour like #ffffff.")
            return { border: { px: borderPx.value, color: borderColor } }
        case "pages": {
            const p = pagesField.text.replace(/\s+/g, "")
            if (!/^\d+(-\d+)?(,\d+(-\d+)?)*$/.test(p))
                return qsTr("List pages like 3,1,2 or 1-4,6.")
            const maxP = pageCount > 0 ? pageCount : pdfOrganizer.knownMaxPages
            if (maxP > 0) {
                for (const n of p.split(/[,-]/))
                    if (parseInt(n, 10) < 1 || parseInt(n, 10) > maxP)
                        return qsTr("This PDF has %n page(s).", "", maxP)
            }
            const expanded = pdfOrganizer.expandSpec(p)
            if (expanded && expanded.length === pdfOrganizer.items.length
                    && pdfOrganizer.hasNonZeroRotation()) {
                return { pages: p, rotations: pdfOrganizer.rotationsList() }
            }
            return { pages: p }
        }
        case "annotate":
            if (annotateEditor.items.length === 0)
                return qsTr("Draw or add a note on the image first.")
            return {
                overlay: annotateEditor.lastExportPath.length > 0
                         ? annotateEditor.lastExportPath
                         : "pending"
            }
        }
        return qsTr("This tool has no options form.")
    }

    function run() {
        if (typeof result === "string")
            return
        if (group === "annotate") {
            const p = annotateEditor.exportOverlay()
            if (!p || p.length === 0)
                return
            App.runWithOptions(toolId, toolTitle, files, JSON.stringify({ overlay: p }))
            win.close()
            return
        }
        App.runWithOptions(toolId, toolTitle, files, JSON.stringify(result))
        win.close()
    }

    // ---- small building blocks -----------------------------------------
    component FieldLabel: Text {
        font.pixelSize: Theme.sizeSmall
        font.weight: Font.DemiBold
        color: Theme.secondaryText
    }
    component NumberField: TextField {
        Layout.preferredWidth: 90
        inputMethodHints: Qt.ImhDigitsOnly
        validator: RegularExpressionValidator { regularExpression: /\d{0,6}/ }
        selectByMouse: true
    }
    component Choice: NButton {
        property bool on: false
        compact: true
        kind: on ? "primary" : "secondary"
    }

    Shortcut { sequence: "Escape"; onActivated: win.close() }
    Shortcut {
        sequences: ["Return", "Enter"]
        enabled: !win.textInputFocused
        onActivated: win.run()
    }
    Shortcut {
        sequence: "Space"
        enabled: win.usesTimeline && !win.textInputFocused
        onActivated: timelineEditor.togglePlay()
    }
    Shortcut {
        sequence: "I"
        enabled: win.usesTimeline && !win.textInputFocused
        onActivated: timelineEditor.setInAtPlayhead()
    }
    Shortcut {
        sequence: "O"
        enabled: win.usesTimeline && !win.textInputFocused
        onActivated: timelineEditor.setOutAtPlayhead()
    }

    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            id: body
            x: 24
            width: Math.max(100, scroll.availableWidth - 48)
            spacing: 14

            PageHeading {
                Layout.fillWidth: true
                Layout.topMargin: 18
                eyebrow: win.files.length > 1 ? qsTr("Applies to %1 files").arg(win.files.length)
                                              : (win.files.length === 1 ? App.fileName(String(win.files[0])) : "")
                title: win.toolTitle
            }

            // Speed
            SectionBox {
                visible: win.group === "speed"
                title: qsTr("Playback speed")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [0.25, 0.5, 0.75, 1.25, 1.5, 2, 3, 4]
                        delegate: Choice {
                            required property real modelData
                            text: modelData + "×"
                            on: win.speed === modelData
                            onClicked: win.speed = modelData
                        }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    visible: win.duration > 0
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                    text: qsTr("New length: about %1").arg(win.formatTime(win.duration / win.speed))
                }
            }

            // Area visual editor + precision inputs (cropImage, cropVideo, redactImage, redactVideo)
            SectionBox {
                visible: win.usesRect
                title: win.group === "crop" ? qsTr("Keep this area (pixels)") : qsTr("Cover these areas (pixels)")

                RectEditor {
                    id: rectEditor
                    Layout.fillWidth: true
                    filePath: win.usesRect ? win.firstFile : ""
                    mode: win.group === "crop" ? "crop" : "redact"
                    isVideo: win.isVideoTool
                    mediaWidth: win.mediaWidth
                    mediaHeight: win.mediaHeight
                    duration: win.duration
                    stageHeight: win.group === "redactVideo" ? 230 : 320

                    onCropChanged: (cx, cy, cw, ch) => {
                        if (win.syncingRect)
                            return
                        win.syncingRect = true
                        rx.text = String(cx)
                        ry.text = String(cy)
                        rw.text = String(cw)
                        rh.text = String(ch)
                        win.syncingRect = false
                    }

                    onRectsModified: {
                        if (win.syncingRect)
                            return
                        win.syncingRect = true
                        if (rectEditor.selectedIndex >= 0 && rectEditor.selectedIndex < rectEditor.rects.length) {
                            const b = rectEditor.rects[rectEditor.selectedIndex]
                            rx.text = String(b.x)
                            ry.text = String(b.y)
                            rw.text = String(b.w)
                            rh.text = String(b.h)
                        } else {
                            rx.text = "0"
                            ry.text = "0"
                            rw.text = ""
                            rh.text = ""
                        }
                        win.syncingRect = false
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    enabled: win.group === "crop" || rectEditor.selectedIndex >= 0 || rectEditor.rects.length === 0

                    GridLayout {
                        columns: 4
                        columnSpacing: 10
                        rowSpacing: 3
                        FieldLabel { text: qsTr("Left") }
                        FieldLabel { text: qsTr("Top") }
                        FieldLabel { text: qsTr("Width") }
                        FieldLabel { text: qsTr("Height") }
                        NumberField {
                            id: rx
                            objectName: "rectLeftField"
                            text: "0"
                            onTextChanged: win.syncRectFromFields()
                        }
                        NumberField {
                            id: ry
                            objectName: "rectTopField"
                            text: "0"
                            onTextChanged: win.syncRectFromFields()
                        }
                        NumberField {
                            id: rw
                            objectName: "rectWidthField"
                            onTextChanged: win.syncRectFromFields()
                        }
                        NumberField {
                            id: rh
                            objectName: "rectHeightField"
                            onTextChanged: win.syncRectFromFields()
                        }
                    }

                    Item { Layout.fillWidth: true }

                    Text {
                        Layout.alignment: Qt.AlignBottom
                        visible: win.mediaWidth > 0
                        font.pixelSize: Theme.sizeSmall
                        color: Theme.secondaryText
                        text: qsTr("Source: %1 × %2").arg(win.mediaWidth).arg(win.mediaHeight)
                    }
                }
            }

            // Timeline visual editor + precision inputs (trimVideo, trimAudio, redactVideo, redactAudio)
            SectionBox {
                visible: win.usesTimeline
                title: win.group === "trim" ? qsTr("Keep this part")
                     : (win.group === "redactAudio" ? qsTr("Mute these parts") : qsTr("When to cover"))

                TimelineEditor {
                    id: timelineEditor
                    Layout.fillWidth: true
                    filePath: win.usesTimeline ? win.firstFile : ""
                    mode: win.group === "redactAudio" ? "ranges" : "trim"
                    showVideoPreview: win.toolId === "trimVideo"
                    duration: win.duration
                    ranges: win.ranges

                    onRangeEdited: (s, e) => {
                        if (win.syncingTime)
                            return
                        win.syncingTime = true
                        startField.text = win.formatTime(s)
                        endField.text = win.formatTime(e)
                        win.syncingTime = false
                    }

                    onRangesEdited: updated => {
                        win.ranges = updated.map(r => [Number(r[0]), Number(r[1])])
                    }
                }

                // Precision Start / End fields for trimVideo, trimAudio, redactVideo
                RowLayout {
                    Layout.fillWidth: true
                    visible: win.usesRange
                    spacing: 12

                    ColumnLayout {
                        spacing: 3
                        FieldLabel { text: qsTr("Start") }
                        TextField {
                            id: startField
                            objectName: "trimStartField"
                            Layout.preferredWidth: 120
                            text: "0"
                            selectByMouse: true
                            onTextChanged: win.syncTimelineFromFields()
                        }
                    }
                    ColumnLayout {
                        spacing: 3
                        FieldLabel { text: qsTr("End") }
                        TextField {
                            id: endField
                            objectName: "trimEndField"
                            Layout.preferredWidth: 120
                            selectByMouse: true
                            placeholderText: win.duration > 0 ? win.formatTime(win.duration) : qsTr("e.g. 1:30")
                            onTextChanged: win.syncTimelineFromFields()
                        }
                    }
                    Item { Layout.fillWidth: true }
                    Text {
                        Layout.alignment: Qt.AlignBottom
                        font.pixelSize: Theme.sizeSmall
                        color: Theme.secondaryText
                        text: win.duration > 0 ? qsTr("Length: %1").arg(win.formatTime(win.duration))
                                               : qsTr("Times as 75, 1:15 or 1:02:03.5")
                    }
                }

                Switch {
                    id: trimSilenceSwitch
                    objectName: "trimSilenceSwitch"
                    visible: win.toolId === "trimAudio"
                    text: qsTr("Remove silence from beginning and end")
                }

                // Precision mute range rows for redactAudio
                ColumnLayout {
                    Layout.fillWidth: true
                    visible: win.group === "redactAudio"
                    spacing: 6

                    RowLayout {
                        spacing: 8
                        Choice {
                            text: qsTr("Mute")
                            on: win.audioMode === "mute"
                            onClicked: win.audioMode = "mute"
                        }
                        Choice {
                            text: qsTr("Bleep")
                            on: win.audioMode === "bleep"
                            onClicked: win.audioMode = "bleep"
                        }
                    }

                    Repeater {
                        model: win.ranges.length
                        delegate: RowLayout {
                            id: rangeRow
                            required property int index
                            spacing: 8

                            readonly property bool selected: index === timelineEditor.selectedRangeIndex

                            function update(slot, text) {
                                const v = win.parseTime(text)
                                const copy = win.ranges.map(r => [Number(r[0]), Number(r[1])])
                                copy[rangeRow.index][slot] = isNaN(v) ? -1 : v
                                win.ranges = copy
                            }

                            Pill {
                                text: "#" + (rangeRow.index + 1)
                                tint: rangeRow.selected ? Theme.goldInk : Theme.forest
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: timelineEditor.selectedRangeIndex = rangeRow.index
                                }
                            }

                            TextField {
                                Layout.preferredWidth: 110
                                text: win.formatTime(win.ranges[rangeRow.index][0])
                                selectByMouse: true
                                onActiveFocusChanged: if (activeFocus) timelineEditor.selectedRangeIndex = rangeRow.index
                                onEditingFinished: rangeRow.update(0, text)
                            }
                            Text { text: "→"; color: Theme.secondaryText }
                            TextField {
                                Layout.preferredWidth: 110
                                text: win.formatTime(win.ranges[rangeRow.index][1])
                                selectByMouse: true
                                onActiveFocusChanged: if (activeFocus) timelineEditor.selectedRangeIndex = rangeRow.index
                                onEditingFinished: rangeRow.update(1, text)
                            }
                            NButton {
                                compact: true
                                kind: "quiet"
                                iconName: "list-remove"
                                enabled: win.ranges.length > 1
                                Accessible.name: qsTr("Remove range")
                                onClicked: {
                                    const next = win.ranges.filter((_, i) => i !== rangeRow.index)
                                    win.ranges = next
                                    if (timelineEditor.selectedRangeIndex >= next.length)
                                        timelineEditor.selectedRangeIndex = Math.max(0, next.length - 1)
                                }
                            }
                        }
                    }
                }
            }

            // Edit photo
            SectionBox {
                visible: win.group === "edit"
                title: qsTr("Rotate and flip")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [[0, qsTr("No rotation")], [90, "90°"], [180, "180°"], [270, "270°"]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.rotate === Number(modelData[0])
                            onClicked: win.rotate = Number(modelData[0])
                        }
                    }
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [["", qsTr("No flip")], ["horizontal", qsTr("Mirror")], ["vertical", qsTr("Upside down")]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.flip === String(modelData[0])
                            onClicked: win.flip = String(modelData[0])
                        }
                    }
                }
            }
            SectionBox {
                visible: win.group === "edit"
                title: qsTr("Resize (optional)")
                RowLayout {
                    spacing: 12
                    ColumnLayout {
                        FieldLabel { text: qsTr("Width") }
                        NumberField { id: resizeW; placeholderText: win.mediaWidth > 0 ? String(win.mediaWidth) : "" }
                    }
                    ColumnLayout {
                        FieldLabel { text: qsTr("Height") }
                        NumberField { id: resizeH; placeholderText: win.mediaHeight > 0 ? String(win.mediaHeight) : "" }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                    text: qsTr("Give one side to keep the proportions.")
                }
            }

            // Frame
            SectionBox {
                visible: win.group === "frame"
                title: qsTr("Background")
                RowLayout {
                    Layout.fillWidth: true
                    FieldLabel { text: qsTr("Thickness") }
                    Slider {
                        id: borderPx
                        Layout.fillWidth: true
                        from: 4
                        to: 400
                        stepSize: 2
                        value: 40
                    }
                    Text {
                        text: qsTr("%1 px").arg(borderPx.value)
                        font.pixelSize: Theme.sizeSmall
                        color: Theme.text
                    }
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 8
                    Repeater {
                        model: ["#ffffff", "#000000", "#f9f6ed", "#e3b859", "#30543b", "#5778ab", "#e8b4b8"]
                        delegate: AbstractButton {
                            id: swatch
                            required property string modelData
                            width: 30
                            height: 30
                            Accessible.name: modelData
                            onClicked: win.borderColor = modelData
                            background: Rectangle {
                                radius: 15
                                color: swatch.modelData
                                border.width: win.borderColor === swatch.modelData ? 3 : 1
                                border.color: win.borderColor === swatch.modelData ? Theme.goldInk : Theme.border
                            }
                        }
                    }
                    TextField {
                        width: 100
                        text: win.borderColor
                        selectByMouse: true
                        onTextEdited: win.borderColor = text.trim()
                    }
                }
            }

            // Organize PDF visual editor + synced page spec input
            SectionBox {
                visible: win.group === "pages"
                title: qsTr("Pages")

                PdfOrganizer {
                    id: pdfOrganizer
                    Layout.fillWidth: true
                    filePath: win.group === "pages" ? win.firstFile : ""
                    pageCount: win.pageCount

                    onPagesEdited: spec => {
                        if (win.syncingPages)
                            return
                        win.syncingPages = true
                        pagesField.text = spec
                        win.syncingPages = false
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    FieldLabel { text: qsTr("Page order") }

                    TextField {
                        id: pagesField
                        objectName: "pdfPagesField"
                        Layout.fillWidth: true
                        selectByMouse: true
                        placeholderText: qsTr("e.g. 3,1,2 or 1-4,6")
                        onTextChanged: {
                            if (win.syncingPages || win.group !== "pages")
                                return
                            win.syncingPages = true
                            pdfOrganizer.syncFromSpec(text)
                            win.syncingPages = false
                        }
                    }
                }

                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                    text: (win.pageCount > 0 ? qsTr("This PDF has %n page(s).", "", win.pageCount) + " " : "")
                          + qsTr("Pages you leave out are dropped; repeat a number to duplicate a page.")
                }
            }

            // Annotate Image visual editor
            SectionBox {
                visible: win.group === "annotate"
                title: qsTr("Markup and notes")

                AnnotateEditor {
                    id: annotateEditor
                    Layout.fillWidth: true
                    filePath: win.group === "annotate" ? win.firstFile : ""
                    mediaWidth: win.mediaWidth
                    mediaHeight: win.mediaHeight
                }
            }

            Text {
                Layout.fillWidth: true
                visible: typeof win.result === "string"
                wrapMode: Text.Wrap
                font.pixelSize: Theme.sizeSmall
                color: Theme.danger
                text: typeof win.result === "string" ? String(win.result) : ""
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.bottomMargin: 20
                spacing: 8
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.sizeTiny
                    color: Theme.tertiaryText
                    text: qsTr("The original stays untouched; a new file is saved next to it.")
                }
                NButton {
                    text: qsTr("Cancel")
                    onClicked: win.close()
                }
                NButton {
                    objectName: "runOptions"
                    kind: "primary"
                    text: qsTr("Save copy")
                    enabled: typeof win.result !== "string"
                    onClicked: win.run()
                }
            }
        }
    }
}
