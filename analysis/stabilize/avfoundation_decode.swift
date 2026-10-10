// swift analysis/stabilize/avfoundation_decode.swift CLIP.mp4
// Opens the clip with AVFoundation, decodes every video frame, and prints the count, size, duration and colour tags.
import AVFoundation

let url = URL(fileURLWithPath: CommandLine.arguments[1])
let asset = AVURLAsset(url: url)
guard let track = try await asset.loadTracks(withMediaType: .video).first else {
	fatalError("no video track")
}
let (size, formats) = try await track.load(.naturalSize, .formatDescriptions)
let duration = try await asset.load(.duration)
let extensions = formats.first.map { CMFormatDescriptionGetExtensions($0) as? [String: Any] ?? [:] } ?? [:]
let reader = try AVAssetReader(asset: asset)
let output = AVAssetReaderTrackOutput(
	track: track, outputSettings: [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA])
reader.add(output)
guard reader.startReading() else { fatalError("startReading: \(String(describing: reader.error))") }
var frames = 0
var times: [Double] = []
while let sample = output.copyNextSampleBuffer() {
	if CMSampleBufferGetImageBuffer(sample) != nil {
		frames += 1
		times.append(CMSampleBufferGetPresentationTimeStamp(sample).seconds)
	}
}
guard reader.status == .completed else { fatalError("reader: \(String(describing: reader.error))") }
print("frames=\(frames) size=\(Int(size.width))x\(Int(size.height)) duration=\(duration.seconds)")
print("first_pts=\(times.first ?? -1) last_pts=\(times.last ?? -1)")
for key in ["CVImageBufferColorPrimaries", "CVImageBufferTransferFunction", "CVImageBufferYCbCrMatrix", "FullRangeVideo"] {
	print("\(key)=\(extensions[key] ?? "absent")")
}
