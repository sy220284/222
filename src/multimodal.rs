//! Bounded, provider-neutral SDK text/image input.
//! Only locally supplied base64 images are accepted; never fetch remote URLs
//! while parsing a message. Keep originals in session storage, not in logs.
use anyhow::{bail,Context,Result};
use serde_json::{json,Value};
const MAX_IMAGE_DATA:usize=5*1024*1024;
const MAX_TOTAL_DATA:usize=12*1024*1024;
const MAX_IMAGES:usize=8;

pub struct Input {
    pub preview:String,
    pub blocks:Option<Vec<Value>>,
}
fn media_type(value:&str)->Result<&str>{
    match value{
        "image/png"|"image/jpeg"|"image/webp"|"image/gif"=>Ok(value),
        _=>bail!("Unsupported image media type"),
    }
}
fn validate_base64(data:&str)->Result<()>{
    if data.is_empty()||data.len()>MAX_IMAGE_DATA||data.len()%4!=0{
        bail!("Image base64 data has an invalid length");
    }
    let mut padding=false;
    let mut pad=0;
    for c in data.bytes(){
        if c==b'=' {padding=true;pad+=1;if pad>2{bail!("Invalid image padding")}}
        else if padding || !(c.is_ascii_alphanumeric()||c==b'+'||c==b'/'){
            bail!("Invalid image base64 encoding");
        }
    }
    Ok(())
}
pub fn parse(value:&Value)->Result<Input>{
    if let Some(text)=value.as_str(){
        if text.len()>MAX_TOTAL_DATA{bail!("SDK text input too large")}
        return Ok(Input{preview:text.to_owned(),blocks:None})
    }
    let entries=value.as_array().context("SDK content must be a string or content blocks")?;
    if entries.is_empty(){bail!("SDK content blocks cannot be empty")}
    let mut normalized=Vec::new();
    let mut preview=Vec::new();
    let mut images=0usize;
    let mut size=0usize;
    for (index,entry) in entries.iter().enumerate(){
        match entry.get("type").and_then(Value::as_str){
            Some("text")=>{
                let text=entry.get("text").and_then(Value::as_str)
                    .with_context(||format!("Content block {index} missing text"))?;
                size=size.saturating_add(text.len());
                normalized.push(json!({"type":"text","text":text}));
                preview.push(text.to_owned());
            },
            Some("image")=>{
                let source=entry.get("source").context("Image requires source")?;
                if source.get("type").and_then(Value::as_str)!=Some("base64"){
                    bail!("SDK image requires a base64 source");
                }
                let mime=media_type(source.get("media_type").and_then(Value::as_str)
                    .context("Image missing media_type")?)?;
                let data=source.get("data").and_then(Value::as_str)
                    .context("Image missing data")?;
                validate_base64(data)?;
                images+=1;
                if images>MAX_IMAGES{bail!("SDK input contains too many images")}
                size=size.saturating_add(data.len());
                normalized.push(json!({"type":"image","mime_type":mime,"data":data}));
                preview.push(format!("[image: {mime}]"));
            },
            _=>bail!("Unsupported SDK content block {index}"),
        }
        if size>MAX_TOTAL_DATA{bail!("SDK content exceeds 12 MB")}
    }
    Ok(Input{preview:preview.join("\n"),blocks:Some(normalized)})
}
pub fn openai(blocks:&[Value])->Result<Vec<Value>>{
    let mut result=Vec::with_capacity(blocks.len());
    for block in blocks{
        match block["type"].as_str(){
            Some("text")=>result.push(json!({"type":"text","text":block["text"]})),
            Some("image")=>{
                let mime=media_type(block["mime_type"].as_str()
                    .context("Image has no media type")?)?;
                let data=block["data"].as_str().context("Image has no data")?;
                validate_base64(data)?;
                result.push(json!({"type":"image_url",
                    "image_url":{"url":format!("data:{mime};base64,{data}")}}));
            },
            _=>bail!("Unsupported content block for OpenAI"),
        }
    }
    Ok(result)
}
pub fn gemini(blocks:&[Value])->Result<Vec<Value>>{
    let mut result=Vec::with_capacity(blocks.len());
    for block in blocks{
        match block["type"].as_str(){
            Some("text")=>result.push(json!({"text":block["text"]})),
            Some("image")=>{
                let mime=media_type(block["mime_type"].as_str()
                    .context("Image has no media type")?)?;
                let data=block["data"].as_str().context("Image has no data")?;
                validate_base64(data)?;
                result.push(json!({"inlineData":{"mimeType":mime,"data":data}}));
            },
            _=>bail!("Unsupported content block for Gemini"),
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn preserves_text_and_images_without_exposing_binary_in_preview(){
        let input=parse(&json!([
            {"type":"text","text":"What is visible?"},
            {"type":"image","source":{"type":"base64","media_type":"image/png","data":"AQID"}}
        ])).unwrap();
        assert!(input.preview.contains("[image: image/png]"));
        assert!(!input.preview.contains("AQID"));
        let blocks=input.blocks.unwrap();
        assert_eq!(openai(&blocks).unwrap()[1]["image_url"]["url"],
            "data:image/png;base64,AQID");
        assert_eq!(gemini(&blocks).unwrap()[1]["inlineData"]["data"],"AQID");
    }
    #[test]
    fn rejects_bad_payload_before_network_or_queue_mutation(){
        for input in [
            json!([{"type":"image","source":{"type":"url","url":"https://example.org/image.png"}}]),
            json!([{"type":"image","source":{"type":"base64","media_type":"application/pdf","data":"AQID"}}]),
            json!([{"type":"image","source":{"type":"base64","media_type":"image/png","data":"INVALID!"}}]),
            json!([{"type":"tool_result","content":"untrusted"}]),
            json!([])
        ]{
            assert!(parse(&input).is_err(),"{input}");
        }
    }
}
