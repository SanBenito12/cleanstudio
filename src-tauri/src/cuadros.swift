// Extrae cuadros de un video en escala de grises y en miniatura, para el detector de parpadeos
// (calidad.rs). Usa AVFoundation, que ya viene con macOS: no hace falta ffmpeg.
//
// Uso: cuadros <video.mp4> <cuadros-por-segundo> <ancho>
// Salida (stdout): una línea "ancho alto cantidad fps\n" y después los cuadros seguidos,
// ancho*alto bytes cada uno (0 = negro, 255 = blanco).
import AVFoundation
import CoreGraphics
import Foundation

let args = CommandLine.arguments
guard args.count == 4, let fps = Double(args[2]), let ancho = Int(args[3]), fps > 0, ancho > 0 else {
    FileHandle.standardError.write("uso: cuadros <video> <fps> <ancho>\n".data(using: .utf8)!)
    exit(2)
}
let asset = AVURLAsset(url: URL(fileURLWithPath: args[1]))
let duracion = CMTimeGetSeconds((try? await asset.load(.duration)) ?? .zero)
guard duracion.isFinite, duracion > 0,
      let pista = try? await asset.loadTracks(withMediaType: .video).first,
      let (natural, transform) = try? await pista.load(.naturalSize, .preferredTransform) else {
    FileHandle.standardError.write("el video no tiene imagen\n".data(using: .utf8)!)
    exit(1)
}
let tam = natural.applying(transform)
let alto = max(1, Int((Double(ancho) * abs(Double(tam.height)) / abs(Double(tam.width))).rounded()))

let gen = AVAssetImageGenerator(asset: asset)
gen.appliesPreferredTrackTransform = true
gen.requestedTimeToleranceBefore = .zero
gen.requestedTimeToleranceAfter = .zero
gen.maximumSize = CGSize(width: ancho * 4, height: alto * 4)

let n = Int(duracion * fps)
let salida = FileHandle.standardOutput
salida.write("\(ancho) \(alto) \(n) \(fps)\n".data(using: .utf8)!)
var buffer = [UInt8](repeating: 0, count: ancho * alto)
let gris = CGColorSpaceCreateDeviceGray()
for i in 0..<n {
    let t = CMTime(seconds: Double(i) / fps, preferredTimescale: 600)
    if let img = try? await gen.image(at: t).image,
       let ctx = CGContext(data: &buffer, width: ancho, height: alto, bitsPerComponent: 8,
                           bytesPerRow: ancho, space: gris, bitmapInfo: CGImageAlphaInfo.none.rawValue) {
        ctx.interpolationQuality = .medium
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: ancho, height: alto))
    }
    // Si un cuadro no se pudo leer, se repite el anterior (no inventa un cambio)
    salida.write(Data(buffer))
}
