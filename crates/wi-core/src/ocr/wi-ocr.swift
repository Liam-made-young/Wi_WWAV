// wi-ocr: reads the text of an image or a PDF on this Mac, with Apple's
// Vision (docs/NOTES.md, "Reading on this Mac"). Nothing leaves the machine.
//
//   wi-ocr <file> <folder>
//
// Each page is written to <folder> as page-N.jpg, upright and no wider than
// it needs to be, and one JSON object is printed:
//
//   {"taken": "2026-10-07T10:22:31", "offset": "-04:00",
//    "pages": [{"text": "…", "confidence": 0.93, "image": "/…/page-1.jpg"}]}
//
// `taken` is the photo's own date (EXIF), when the file has one. A file that
// can't be read exits 1 with one sentence on stderr.

import CoreGraphics
import Foundation
import ImageIO
import Vision

let longestSide = 2400
let pageLimit = 40

func fail(_ sentence: String) -> Never {
    FileHandle.standardError.write((sentence + "\n").data(using: .utf8)!)
    exit(1)
}

/// The text on one page, top to bottom, and how sure Vision was of it.
func read(_ image: CGImage) -> (String, Double) {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = true
    if #available(macOS 13.0, *) {
        request.automaticallyDetectsLanguage = true
    }
    let wanted = ["en-US", "ja-JP"]
    if let supported = try? request.supportedRecognitionLanguages() {
        let languages = wanted.filter { supported.contains($0) }
        if !languages.isEmpty {
            request.recognitionLanguages = languages
        }
    }
    let handler = VNImageRequestHandler(cgImage: image, options: [:])
    do {
        try handler.perform([request])
    } catch {
        return ("", 0)
    }
    var lines: [String] = []
    var sure: [Double] = []
    for observation in request.results ?? [] {
        guard let best = observation.topCandidates(1).first else { continue }
        lines.append(best.string)
        sure.append(Double(best.confidence))
    }
    let confidence = sure.isEmpty ? 0 : sure.reduce(0, +) / Double(sure.count)
    return (lines.joined(separator: "\n"), confidence)
}

func writeJPEG(_ image: CGImage, to url: URL) -> Bool {
    guard let out = CGImageDestinationCreateWithURL(url as CFURL, "public.jpeg" as CFString, 1, nil) else {
        return false
    }
    CGImageDestinationAddImage(out, image, [kCGImageDestinationLossyCompressionQuality: 0.82] as CFDictionary)
    return CGImageDestinationFinalize(out)
}

/// A PDF's pages, each drawn on white at a size Vision reads well.
func pdfPages(_ url: URL) -> [CGImage]? {
    guard let document = CGPDFDocument(url as CFURL), document.numberOfPages > 0 else { return nil }
    var pages: [CGImage] = []
    for number in 1...min(document.numberOfPages, pageLimit) {
        guard let page = document.page(at: number) else { continue }
        let box = page.getBoxRect(.cropBox)
        let turned = page.rotationAngle % 180 != 0
        let (w, h) = turned ? (box.height, box.width) : (box.width, box.height)
        guard w > 0, h > 0 else { continue }
        let scale = CGFloat(longestSide) / max(w, h)
        let (pw, ph) = (Int((w * scale).rounded()), Int((h * scale).rounded()))
        guard let context = CGContext(
            data: nil, width: pw, height: ph, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
        else { continue }
        context.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: pw, height: ph))
        context.interpolationQuality = .high
        let fit = page.getDrawingTransform(.cropBox, rect: CGRect(x: 0, y: 0, width: w, height: h), rotate: 0, preserveAspectRatio: true)
        context.scaleBy(x: scale, y: scale)
        context.concatenate(fit)
        context.drawPDFPage(page)
        if let image = context.makeImage() {
            pages.append(image)
        }
    }
    return pages.isEmpty ? nil : pages
}

let arguments = CommandLine.arguments
guard arguments.count == 3 else { fail("usage: wi-ocr <file> <folder>") }
let file = URL(fileURLWithPath: arguments[1])
let folder = URL(fileURLWithPath: arguments[2], isDirectory: true)
try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)

var images: [CGImage] = []
var answer: [String: Any] = [:]

if file.pathExtension.lowercased() == "pdf" {
    guard let pages = pdfPages(file) else { fail("That PDF has no pages Learn can read.") }
    images = pages
} else {
    guard let source = CGImageSourceCreateWithURL(file as CFURL, nil), CGImageSourceGetCount(source) > 0 else {
        fail("That file isn't an image Learn can read.")
    }
    // Upright, whatever way the phone was held.
    let options: [CFString: Any] = [
        kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceCreateThumbnailWithTransform: true,
        kCGImageSourceThumbnailMaxPixelSize: longestSide,
    ]
    guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
        fail("That file isn't an image Learn can read.")
    }
    images = [image]
    if let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
        let exif = properties[kCGImagePropertyExifDictionary] as? [CFString: Any],
        let taken = exif[kCGImagePropertyExifDateTimeOriginal] as? String
    {
        // "2026:10:07 10:22:31" as "2026-10-07T10:22:31".
        let parts = taken.split(separator: " ")
        if parts.count == 2 {
            answer["taken"] = parts[0].replacingOccurrences(of: ":", with: "-") + "T" + parts[1]
        }
        if let offset = exif[kCGImagePropertyExifOffsetTimeOriginal] as? String {
            answer["offset"] = offset
        }
    }
}

var pages: [[String: Any]] = []
for (index, image) in images.enumerated() {
    let (text, confidence) = read(image)
    let out = folder.appendingPathComponent("page-\(index + 1).jpg")
    guard writeJPEG(image, to: out) else { fail("Learn couldn't write the page's image.") }
    pages.append(["text": text, "confidence": confidence, "image": out.path])
}
answer["pages"] = pages

guard let json = try? JSONSerialization.data(withJSONObject: answer, options: []) else {
    fail("Learn couldn't write what it read.")
}
FileHandle.standardOutput.write(json)
FileHandle.standardOutput.write("\n".data(using: .utf8)!)
