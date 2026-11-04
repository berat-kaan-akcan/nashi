pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Quick-look style preview window for Library items (images, GIFs, PDFs, text,
// audio/video via dynamic QtMultimedia + waveform/frame fallback, or file card).
ApplicationWindow {
    id: root

    property var files: []
    property int currentIndex: -1
    readonly property string filePath: currentIndex >= 0 && currentIndex < files.length ? String(files[currentIndex]) : ""
    property bool fileExists: false
    property string viewMode: "empty" // empty | missing | image | gif | pdf | text | video | audio | card
    property var fileInfo: ({})
    property bool loading: false
    property string previewError: ""

    property int inspectReqId: -1
    property int previewReqId: -1
    property var pageImages: []
    property int loadedPageLimit: 8
    property string frameImage: ""
    property string waveformImage: ""
    property string textContent: ""

    // Dynamic QtMultimedia Video or MediaPlayer instance (null when unavailable or headless).
    property var mediaPlayer: null
    property bool mediaPlaying: false
    property real mediaPositionMs: 0
    property real mediaDurationMs: 0

    readonly property int totalPages: fileInfo && fileInfo.pages ? Number(fileInfo.pages) : 0
    readonly property bool hasPrev: currentIndex > 0
    readonly property bool hasNext: currentIndex >= 0 && currentIndex < files.length - 1

    function isPlainText(category, ext) {
        if (category === "text" || category === "subtitle")
            return true
        const plainExts = [
            "txt", "md", "markdown", "json", "csv", "tsv", "srt", "vtt",
            "html", "htm", "xml", "yaml", "yml", "toml", "ini", "conf", "log", "tex"
        ]
        return plainExts.indexOf(ext) >= 0
    }

    function formatDuration(sec) {
        if (!isFinite(sec) || sec < 0)
            return "0:00"
        const total = Math.floor(sec)
        const hrs = Math.floor(total / 3600)
        const mins = Math.floor((total % 3600) / 60)
        const secs = total % 60
        const pad = secs < 10 ? "0" + secs : String(secs)
        if (hrs > 0)
            return hrs + ":" + (mins < 10 ? "0" + mins : String(mins)) + ":" + pad
        return mins + ":" + pad
    }

    function metadataSummary() {
        if (filePath.length === 0)
            return ""
        const parts = []
        if (fileInfo && fileInfo.width && fileInfo.height)
            parts.push(fileInfo.width + "×" + fileInfo.height)
        if (fileInfo && fileInfo.duration !== undefined && fileInfo.duration !== null)
            parts.push(formatDuration(Number(fileInfo.duration)))
        if (totalPages > 0)
            parts.push(qsTr("%n page(s)", "", totalPages))
        const bytes = fileInfo && fileInfo.size ? Number(fileInfo.size) : App.fileSize(filePath)
        if (bytes > 0)
            parts.push(App.formatSize(bytes))
        return parts.join(" · ")
    }

    function destroyPlayer() {
        if (mediaPlayer) {
            try {
                mediaPlayer.stop()
                mediaPlayer.destroy()
            } catch (e) {
            }
            mediaPlayer = null
        }
        mediaPlaying = false
        mediaPositionMs = 0
        mediaDurationMs = 0
    }

    function ensureMediaPlayer(kind) {
        if (mediaPlayer || !visible || !fileExists || filePath.length === 0 || !App.multimediaAvailable)
            return
        try {
            const parentItem = kind === "video" ? videoHost : audioHost
            const qml = kind === "video"
                ? 'import QtQuick\nimport QtMultimedia\nVideo { anchors.fill: parent; fillMode: VideoOutput.PreserveAspectFit }\n'
                : 'import QtQuick\nimport QtMultimedia\nMediaPlayer { audioOutput: AudioOutput {} }\n'
            const player = Qt.createQmlObject(qml, parentItem, "NashiDynamicMediaPlayer")
            if (!player)
                return
            player.source = App.localFileUrl(filePath)
            player.playbackStateChanged.connect(function() {
                if (root.mediaPlayer === player)
                    root.mediaPlaying = (player.playbackState === 1)
            })
            player.positionChanged.connect(function() {
                if (root.mediaPlayer === player && !seekSlider.pressed)
                    root.mediaPositionMs = player.position
            })
            player.durationChanged.connect(function() {
                if (root.mediaPlayer === player && player.duration > 0)
                    root.mediaDurationMs = player.duration
            })
            if (fileInfo && fileInfo.duration)
                mediaDurationMs = Number(fileInfo.duration) * 1000
            mediaPlayer = player
        } catch (e) {
            mediaPlayer = null
        }
    }

    function togglePlayPause() {
        if (!mediaPlayer && (viewMode === "video" || viewMode === "audio"))
            ensureMediaPlayer(viewMode)
        if (!mediaPlayer)
            return
        try {
            if (mediaPlayer.playbackState === 1)
                mediaPlayer.pause()
            else
                mediaPlayer.play()
        } catch (e) {
        }
    }

    function requestPdfPages() {
        if (filePath.length === 0)
            return
        const limit = totalPages > 0 ? Math.min(totalPages, loadedPageLimit) : loadedPageLimit
        loading = true
        previewReqId = App.preview.request(filePath, "pages", {
            pages: "1-" + Math.max(1, limit),
            width: 800
        })
    }

    function loadMorePages() {
        loadedPageLimit += 8
        requestPdfPages()
    }

    function requestFallbackFrame() {
        if (filePath.length === 0 || previewReqId > 0)
            return
        loading = true
        previewReqId = App.preview.request(filePath, "frame", { width: 1400 })
    }

    function openPreview(fileList, idx) {
        viewMode = "card"
        frameImage = ""
        const arr = []
        if (fileList) {
            for (let i = 0; i < fileList.length; ++i)
                arr.push(String(fileList[i]))
        }
        files = arr
        currentIndex = (arr.length > 0) ? Math.max(0, Math.min(idx, arr.length - 1)) : -1
        loadCurrent()
    }

    function prev() {
        if (hasPrev) {
            viewMode = "card"
            frameImage = ""
            currentIndex -= 1
            loadCurrent()
        }
    }

    function next() {
        if (hasNext) {
            viewMode = "card"
            frameImage = ""
            currentIndex += 1
            loadCurrent()
        }
    }

    function loadCurrent() {
        destroyPlayer()
        fileInfo = ({})
        pageImages = []
        loadedPageLimit = 8
        frameImage = ""
        waveformImage = ""
        textContent = ""
        previewError = ""
        inspectReqId = -1
        previewReqId = -1
        loading = false

        if (filePath.length === 0) {
            fileExists = false
            viewMode = "empty"
            return
        }
        fileExists = App.fileExists(filePath)
        if (!fileExists) {
            viewMode = "missing"
            return
        }
        loading = true
        viewMode = "card"
        inspectReqId = App.preview.inspect(filePath)
    }

    visible: false
    width: 880
    height: 640
    minimumWidth: 560
    minimumHeight: 420
    title: filePath.length > 0 ? App.fileName(filePath) + " — " + qsTr("Preview") : qsTr("Preview")
    color: Theme.canvas

    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.surface
    palette.alternateBase: Theme.inset
    palette.text: Theme.text
    palette.button: Theme.surface
    palette.buttonText: Theme.text
    palette.highlight: Theme.gold
    palette.highlightedText: Theme.buttonInk
    palette.placeholderText: Theme.tertiaryText
    palette.mid: Theme.border

    onClosing: destroyPlayer()
    onVisibleChanged: {
        if (!visible)
            destroyPlayer()
        else if (viewMode === "video" || viewMode === "audio")
            ensureMediaPlayer(viewMode)
    }

    Shortcut { sequence: "Escape"; onActivated: root.close() }
    Shortcut { sequence: "Left"; enabled: root.hasPrev; onActivated: root.prev() }
    Shortcut { sequence: "Right"; enabled: root.hasNext; onActivated: root.next() }
    Shortcut {
        sequence: "Space"
        enabled: root.viewMode === "video" || root.viewMode === "audio"
        onActivated: root.togglePlayPause()
    }

    Connections {
        target: App.preview

        function onInspected(requestId, info) {
            if (requestId !== root.inspectReqId)
                return
            root.fileInfo = info || ({})
            root.loading = false

            const cat = String(root.fileInfo.category || "unknown")
            const ext = String(root.fileInfo.ext || "").toLowerCase()

            if (root.isPlainText(cat, ext)) {
                root.viewMode = "text"
                root.textContent = App.preview.readTextPreview(root.filePath, 65536)
            } else if (cat === "gif") {
                root.viewMode = "gif"
            } else if (cat === "image") {
                root.viewMode = "image"
            } else if (cat === "pdf") {
                root.viewMode = "pdf"
                root.requestPdfPages()
            } else if (cat === "video") {
                root.viewMode = "video"
                root.mediaDurationMs = root.fileInfo.duration ? Number(root.fileInfo.duration) * 1000 : 0
                root.previewReqId = App.preview.request(root.filePath, "frame", { width: 1280 })
                root.ensureMediaPlayer("video")
            } else if (cat === "audio") {
                root.viewMode = "audio"
                root.mediaDurationMs = root.fileInfo.duration ? Number(root.fileInfo.duration) * 1000 : 0
                root.previewReqId = App.preview.request(root.filePath, "waveform", {
                    width: 960,
                    height: 140,
                    color: Theme.dark ? "#b3d19e" : "#30543b"
                })
                root.ensureMediaPlayer("audio")
            } else {
                root.viewMode = "card"
            }
        }

        function onReady(requestId, kind, outFiles, width, height) {
            if (requestId !== root.previewReqId)
                return
            root.loading = false
            if (kind === "pages") {
                root.pageImages = outFiles || []
            } else if (kind === "waveform") {
                root.waveformImage = (outFiles && outFiles.length > 0) ? String(outFiles[0]) : ""
            } else if (kind === "frame") {
                root.frameImage = (outFiles && outFiles.length > 0) ? String(outFiles[0]) : ""
            }
        }

        function onFailed(requestId, message) {
            if (requestId === root.inspectReqId) {
                root.loading = false
                root.previewError = message
                root.viewMode = "card"
            } else if (requestId === root.previewReqId) {
                root.loading = false
                root.previewError = message
            }
        }
    }

    // Header bar
    header: Rectangle {
        implicitHeight: headerRow.implicitHeight + 24
        color: Theme.canvas

        RowLayout {
            id: headerRow
            anchors.fill: parent
            anchors.leftMargin: 20
            anchors.rightMargin: 20
            anchors.topMargin: 14
            anchors.bottomMargin: 10
            spacing: 10

            NButton {
                objectName: "previewPrevButton"
                visible: root.files.length > 1
                compact: true
                kind: "secondary"
                iconName: "go-previous"
                Accessible.name: qsTr("Previous")
                enabled: root.hasPrev
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Previous · ←")
                onClicked: root.prev()
            }
            NButton {
                objectName: "previewNextButton"
                visible: root.files.length > 1
                compact: true
                kind: "secondary"
                iconName: "go-next"
                Accessible.name: qsTr("Next")
                enabled: root.hasNext
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Next · →")
                onClicked: root.next()
            }
            Pill {
                visible: root.files.length > 1
                text: (root.currentIndex + 1) + " / " + root.files.length
                tint: Theme.gold
                textColor: Theme.goldInk
            }

            Rectangle {
                visible: root.filePath.length > 0
                Layout.preferredWidth: 32
                Layout.preferredHeight: 32
                radius: 9
                color: Theme.inset
                Image {
                    anchors.centerIn: parent
                    source: root.filePath.length > 0 ? "image://icon/" + App.mimeIcon(root.filePath) : ""
                    sourceSize: Qt.size(40, 40)
                    width: 20
                    height: 20
                }
            }

            Text {
                Layout.fillWidth: true
                text: root.filePath.length > 0 ? App.fileName(root.filePath) : qsTr("No file selected")
                elide: Text.ElideMiddle
                font.pixelSize: Theme.sizeHeading
                font.weight: Font.Bold
                color: Theme.text
            }

            Pill {
                objectName: "previewMetadataPill"
                visible: text.length > 0
                text: root.metadataSummary()
                tint: Theme.forest
            }
        }
    }

    // Main stage
    Rectangle {
        id: stage
        anchors.fill: parent
        anchors.leftMargin: 20
        anchors.rightMargin: 20
        anchors.topMargin: 2
        anchors.bottomMargin: 2
        radius: Theme.radiusCard
        color: Theme.surface
        border.width: 1
        border.color: Theme.border
        clip: true

        // 1. Empty / Missing state
        ColumnLayout {
            anchors.centerIn: parent
            width: Math.min(420, parent.width - 48)
            visible: root.viewMode === "empty" || root.viewMode === "missing"
            spacing: 12

            NashiMark {
                Layout.alignment: Qt.AlignHCenter
                size: 72
                happy: false
            }
            Text {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                font.pixelSize: Theme.sizeHeading
                font.weight: Font.DemiBold
                color: Theme.text
                text: root.viewMode === "missing" ? qsTr("This file is no longer at its original location.")
                                                  : qsTr("Select a file in your Library to preview it.")
            }
        }

        // 2. Raster image preview
        Item {
            anchors.fill: parent
            anchors.margins: 16
            visible: root.viewMode === "image"

            Image {
                id: directImage
                objectName: "previewRasterImage"
                anchors.fill: parent
                autoTransform: true
                asynchronous: false
                fillMode: Image.PreserveAspectFit
                source: {
                    if (root.viewMode !== "image" || root.filePath.length === 0)
                        return ""
                    if (root.frameImage.length > 0)
                        return App.localFileUrl(root.frameImage)
                    return App.localFileUrl(root.filePath)
                }
                onStatusChanged: {
                    if (status === Image.Error && root.frameImage.length === 0 && root.viewMode === "image")
                        root.requestFallbackFrame()
                }
            }
        }

        // 3. Animated GIF preview
        Item {
            anchors.fill: parent
            anchors.margins: 16
            visible: root.viewMode === "gif"

            AnimatedImage {
                anchors.fill: parent
                visible: root.frameImage.length === 0
                playing: root.visible && root.viewMode === "gif"
                fillMode: Image.PreserveAspectFit
                source: (root.viewMode === "gif" && root.frameImage.length === 0 && root.filePath.length > 0)
                        ? App.localFileUrl(root.filePath) : ""
                onStatusChanged: {
                    if (status === Image.Error && root.frameImage.length === 0 && root.viewMode === "gif")
                        root.requestFallbackFrame()
                }
            }
            Image {
                anchors.fill: parent
                visible: root.frameImage.length > 0
                fillMode: Image.PreserveAspectFit
                source: (root.viewMode === "gif" && root.frameImage.length > 0)
                        ? App.localFileUrl(root.frameImage) : ""
            }
        }

        // 4. PDF scrollable page view
        ScrollView {
            id: pdfScroll
            objectName: "previewPdfView"
            anchors.fill: parent
            visible: root.viewMode === "pdf"
            contentWidth: availableWidth

            ColumnLayout {
                width: Math.max(200, pdfScroll.availableWidth)
                spacing: 18

                Item { Layout.preferredHeight: 8 }

                Repeater {
                    model: root.pageImages
                    delegate: ColumnLayout {
                        id: pageDelegate
                        required property int index
                        required property string modelData
                        Layout.alignment: Qt.AlignHCenter
                        Layout.fillWidth: true
                        spacing: 6

                        Rectangle {
                            Layout.alignment: Qt.AlignHCenter
                            readonly property real maxW: Math.min(720, Math.max(220, pdfScroll.availableWidth - 48))
                            readonly property real ratio: pageImg.implicitWidth > 0 ? (pageImg.implicitHeight / pageImg.implicitWidth) : 1.414
                            width: maxW
                            height: Math.round(maxW * ratio)
                            radius: 8
                            color: "#ffffff"
                            border.width: 1
                            border.color: Theme.border
                            clip: true

                            Image {
                                id: pageImg
                                anchors.fill: parent
                                anchors.margins: 1
                                fillMode: Image.PreserveAspectFit
                                asynchronous: false
                                source: pageDelegate.modelData.length > 0 ? App.localFileUrl(pageDelegate.modelData) : ""
                            }
                        }
                        Pill {
                            Layout.alignment: Qt.AlignHCenter
                            text: root.totalPages > 0
                                  ? qsTr("Page %1 of %2").arg(pageDelegate.index + 1).arg(root.totalPages)
                                  : qsTr("Page %1").arg(pageDelegate.index + 1)
                            tint: Theme.forest
                        }
                    }
                }

                NButton {
                    objectName: "loadMorePagesButton"
                    Layout.alignment: Qt.AlignHCenter
                    visible: root.totalPages > root.pageImages.length && root.pageImages.length > 0 && !root.loading
                    kind: "secondary"
                    iconName: "go-down"
                    text: qsTr("Load more pages (%1 remaining)").arg(root.totalPages - root.pageImages.length)
                    onClicked: root.loadMorePages()
                }

                Item { Layout.preferredHeight: 12 }
            }
        }

        // 5. Monospace text / code / subtitle preview
        ScrollView {
            id: textScroll
            objectName: "previewTextView"
            anchors.fill: parent
            anchors.margins: 14
            visible: root.viewMode === "text"

            TextArea {
                objectName: "previewTextArea"
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                textFormat: TextEdit.PlainText
                font.family: "monospace"
                font.pixelSize: Theme.sizeBody
                color: Theme.text
                selectionColor: Theme.gold
                selectedTextColor: Theme.buttonInk
                background: Rectangle {
                    radius: Theme.radiusRow
                    color: Theme.inset
                }
                padding: 16
                text: root.textContent
            }
        }

        // 6. Video & Audio preview (with dynamic QtMultimedia + poster/waveform fallback)
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            visible: root.viewMode === "video" || root.viewMode === "audio"
            spacing: 14

            // Visual area
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: Theme.radiusRow
                color: root.viewMode === "video" ? "#0d120f" : Theme.inset
                border.width: 1
                border.color: Theme.border
                clip: true

                // Poster frame fallback for video (shown behind or when player hasn't rendered)
                Image {
                    anchors.fill: parent
                    anchors.margins: 8
                    visible: root.viewMode === "video" && root.frameImage.length > 0 && !root.mediaPlaying
                    fillMode: Image.PreserveAspectFit
                    source: (root.viewMode === "video" && root.frameImage.length > 0)
                            ? App.localFileUrl(root.frameImage) : ""
                }

                // Dynamic Video host
                Item {
                    id: videoHost
                    anchors.fill: parent
                    visible: root.viewMode === "video"
                }

                // Audio card + waveform
                ColumnLayout {
                    anchors.centerIn: parent
                    width: Math.min(680, parent.width - 40)
                    visible: root.viewMode === "audio"
                    spacing: 16

                    Item { id: audioHost; width: 0; height: 0 }

                    Rectangle {
                        Layout.alignment: Qt.AlignHCenter
                        width: 68
                        height: 68
                        radius: 20
                        color: Theme.sage
                        Image {
                            anchors.centerIn: parent
                            source: "image://icon/audio-x-generic"
                            sourceSize: Qt.size(72, 72)
                            width: 36
                            height: 36
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        elide: Text.ElideMiddle
                        text: root.filePath.length > 0 ? App.fileName(root.filePath) : ""
                        font.pixelSize: Theme.sizeHeading
                        font.weight: Font.Bold
                        color: Theme.text
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 120
                        radius: Theme.radiusRow
                        color: Theme.surface
                        border.width: 1
                        border.color: Theme.border
                        clip: true

                        Image {
                            anchors.fill: parent
                            anchors.margins: 10
                            visible: root.waveformImage.length > 0
                            fillMode: Image.PreserveAspectFit
                            source: (root.viewMode === "audio" && root.waveformImage.length > 0)
                                    ? App.localFileUrl(root.waveformImage) : ""
                        }

                        // Playhead indicator over waveform
                        Rectangle {
                            visible: root.mediaDurationMs > 0 && root.waveformImage.length > 0
                            x: 10 + (parent.width - 20) * Math.max(0, Math.min(1, root.mediaPositionMs / Math.max(1, root.mediaDurationMs)))
                            y: 8
                            width: 2
                            height: parent.height - 16
                            radius: 1
                            color: Theme.gold
                        }
                    }
                }
            }

            // Transport bar
            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                NButton {
                    objectName: "mediaPlayPauseButton"
                    compact: true
                    kind: "primary"
                    enabled: root.mediaPlayer !== null || App.multimediaAvailable
                    iconName: root.mediaPlaying ? "media-playback-pause" : "media-playback-start"
                    text: root.mediaPlaying ? qsTr("Pause") : qsTr("Play")
                    onClicked: root.togglePlayPause()
                }

                Text {
                    font.family: "monospace"
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                    text: root.formatDuration(root.mediaPositionMs / 1000)
                }

                Slider {
                    id: seekSlider
                    Layout.fillWidth: true
                    enabled: root.mediaPlayer !== null && root.mediaDurationMs > 0
                    from: 0
                    to: Math.max(1, root.mediaDurationMs)
                    value: root.mediaPositionMs
                    onMoved: {
                        root.mediaPositionMs = value
                        if (root.mediaPlayer)
                            root.mediaPlayer.position = value
                    }
                }

                Text {
                    font.family: "monospace"
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                    text: root.formatDuration(root.mediaDurationMs / 1000)
                }

                Pill {
                    visible: !App.multimediaAvailable && root.mediaPlayer === null
                    text: qsTr("Static preview")
                    tint: Theme.goldInk
                }
            }
        }

        // 7. Generic file card for archives / binary office documents / unknown
        ColumnLayout {
            anchors.centerIn: parent
            width: Math.min(460, parent.width - 48)
            visible: root.viewMode === "card" && !root.loading
            spacing: 14

            Rectangle {
                Layout.alignment: Qt.AlignHCenter
                width: 76
                height: 76
                radius: 22
                color: Theme.sage
                Image {
                    anchors.centerIn: parent
                    source: root.filePath.length > 0 ? "image://icon/" + App.mimeIcon(root.filePath) : ""
                    sourceSize: Qt.size(80, 80)
                    width: 42
                    height: 42
                }
            }

            Text {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideMiddle
                text: root.filePath.length > 0 ? App.fileName(root.filePath) : ""
                font.pixelSize: Theme.sizeTitle - 3
                font.weight: Font.Bold
                color: Theme.text
            }

            Text {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                elide: Text.ElideMiddle
                maximumLineCount: 2
                text: root.filePath
                font.pixelSize: Theme.sizeSmall
                color: Theme.secondaryText
            }

            Pill {
                Layout.alignment: Qt.AlignHCenter
                visible: text.length > 0
                text: root.metadataSummary()
                tint: Theme.forest
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 4
                spacing: 8
                NButton {
                    kind: "primary"
                    iconName: "document-open"
                    text: qsTr("Open")
                    enabled: root.fileExists
                    onClicked: App.actions.open(root.filePath)
                }
                NButton {
                    kind: "secondary"
                    iconName: "folder-open"
                    text: qsTr("Show in folder")
                    enabled: root.fileExists
                    onClicked: App.actions.reveal([root.filePath])
                }
                NButton {
                    kind: "secondary"
                    iconName: "view-refresh"
                    text: qsTr("Use in Nashi")
                    enabled: root.fileExists
                    onClicked: {
                        App.reuseFiles([root.filePath])
                        root.close()
                    }
                }
            }
        }

        // Loading indicator
        BusyIndicator {
            anchors.centerIn: parent
            visible: running
            running: root.loading
        }
    }

    // Footer action bar
    footer: Rectangle {
        implicitHeight: footerRow.implicitHeight + 24
        color: Theme.canvas

        RowLayout {
            id: footerRow
            anchors.fill: parent
            anchors.leftMargin: 20
            anchors.rightMargin: 20
            anchors.topMargin: 10
            anchors.bottomMargin: 14
            spacing: 8

            Text {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                font.pixelSize: Theme.sizeSmall
                color: root.previewError.length > 0 ? Theme.danger : Theme.secondaryText
                text: root.previewError.length > 0 ? root.previewError
                      : root.filePath.length > 0 ? root.filePath.substring(0, Math.max(0, root.filePath.lastIndexOf("/")))
                      : ""
            }

            NButton {
                objectName: "previewUseButton"
                compact: true
                kind: "primary"
                iconName: "view-refresh"
                text: qsTr("Use in Nashi")
                enabled: root.fileExists
                onClicked: {
                    App.reuseFiles([root.filePath])
                    root.close()
                }
            }
            NButton {
                objectName: "previewPinButton"
                compact: true
                kind: "secondary"
                iconName: "pin"
                text: qsTr("Add to Float")
                enabled: root.fileExists
                onClicked: App.addToShelf([root.filePath])
            }
            NButton {
                objectName: "previewShareButton"
                visible: App.shareAvailable
                compact: true
                kind: "secondary"
                iconName: "document-send"
                text: qsTr("Share")
                enabled: root.fileExists
                onClicked: App.showShare([root.filePath])
            }
            NButton {
                compact: true
                kind: "secondary"
                iconName: "folder-open"
                text: qsTr("Show in folder")
                enabled: root.fileExists
                onClicked: App.actions.reveal([root.filePath])
            }
            NButton {
                compact: true
                kind: "secondary"
                iconName: "document-open"
                text: qsTr("Open")
                enabled: root.fileExists
                onClicked: App.actions.open(root.filePath)
            }
        }
    }
}
