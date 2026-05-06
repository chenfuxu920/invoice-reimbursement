from fastapi import FastAPI, UploadFile, File
from ocr_engine import recognize_image

app = FastAPI(title="Invoice OCR Service", version="0.1.0")

@app.get("/health")
async def health():
    return {"status": "ok"}

@app.post("/ocr/image")
async def ocr_image(file: UploadFile = File(...)):
    """识别上传的图片"""
    image_bytes = await file.read()
    result = recognize_image(image_bytes)
    return {"texts": result}

@app.post("/ocr/pdf")
async def ocr_pdf(file: UploadFile = File(...)):
    """识别上传的 PDF（转为图片后 OCR）"""
    from pdf2image import convert_from_bytes
    import io

    pdf_bytes = await file.read()
    images = convert_from_bytes(pdf_bytes)

    all_texts = []
    for i, img in enumerate(images):
        buf = io.BytesIO()
        img.save(buf, format='PNG')
        result = recognize_image(buf.getvalue())
        all_texts.append({
            'page': i + 1,
            'texts': result,
        })
    return {"pages": all_texts}
